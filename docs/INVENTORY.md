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

## Fitness — `/fitness` (private, multi-user)

A nutrition and weight tracker for several people, session-gated, dark-themed.
Each person has their own log; the food catalog is shared.

- Today view: calorie/macro targets ring, week strip, meals by slot
  (breakfast/lunch/dinner/snack/other).
- Week view: calorie bars, protein stats, logging streak, weight trend,
  most-logged foods.
- Food database with favourites, custom portions, and recipes; quick-log,
  copy-day, and barcode scanning that matches known products or prefills
  new ones from OpenFoodFacts.
- Accounts: the owner signs in with a passkey; everyone else with a name and
  a PIN, created for them at `/admin/users`. There is no public sign-up.
  Entries, weights, targets and recipes are private to each person; the food
  database, favourites and portions aside, is common.
- Art-portfolio admin is a permission the owner grants and revokes per
  account — a fitness account reaches `/fitness` and nothing else. Only the
  owner manages accounts, and the owner cannot be demoted or deleted.
- Everyone manages their own name and PIN at `/fitness/account`.
- The Today screen logs food in one tap. Each logged row carries its own
  amount controls — fractions of whatever that food comes in, so half a pack
  is a tap rather than a sum — and re-logging, saved meals and "usual at this
  meal" are each a single tap. Typing a food nobody has entered yet creates it
  and logs it in the same tap, macros fillable later.
- The day says where it stands without arithmetic: calories left, a macro
  composition pie, what is still owed against each target, and a streak.
- On a phone it is one column with Scan, Search and copy-yesterday in thumb
  reach; `/` jumps to the search field and `Esc` backs out one layer.

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
- Mobile play flow (2026-08-13 redesign): HAND is a reading wheel — tap
  the focused card for a full-rules inspect sheet whose PLAY button jumps
  to the TABLE; TABLE is the play surface — drag or tap a tray card onto
  a target, armed plays queue on the felt with dotted arrows until LOCK
  IN; card swaps happen in a full-screen mulligan overlay; seat chips
  show everyone's per-deck hand counts; a mode badge + pull count ride
  the tab row. Container: `docs/manifests/2026-08-13-lc-mobile-play-flow.md`
- 🚧 Challenge cards — real-life party challenges as Last Call cards
  (duels judged by table vote, solo dares, social penalties).
  Container: `docs/manifests/2026-08-14-lc-challenge-cards.md`

## Infrastructure

Single Rust/Axum binary, server-side rendered, SQLite storage, S3-compatible
object storage for images, passkey (WebAuthn) authentication for the single
admin. Deployed to a Hetzner server via GitHub Actions on push to master,
behind nginx. Quality gates: `scripts/check.sh` (item) and
`scripts/verify.sh` (pack).

---

*Nothing in transit right now.*
