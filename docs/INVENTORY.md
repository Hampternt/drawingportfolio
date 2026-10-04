# Inventory

The live map of what this repo **is**, at feature altitude: what exists and
what it does — never how it's implemented (that lives in `CLAUDE.md` and
`docs/design.md`). Every entry opens with its state: ✅ shipped, 🚧 in
flight, 💭 considered. Work in flight appears as a 🚧 pointer to its manifest in
the section where it will land; converting that placeholder into a ✅ entry is
part of the definition of merged. **Considered**, at the foot, records what the
owner has raised but not scheduled — one line each, no dates, no ordering.
Manifests live in `docs/manifests/`.

---

## Hub — `/`

The landing page: entry point linking to every public area of the site.

- ✅ A dark front door on the house design system: a blueprint-grid hero naming
  what the site runs on, the tagline, and who runs it — the owner's short name
  and one line on what they do.
- ✅ A search bar that opens the command palette on click, so every destination on
  the site is reachable without knowing a URL. `Ctrl`+`K` does the same from any
  page.
- ✅ Six section tiles — drawing portfolio, drawing tasks, drinks, fitness,
  sorting, CV — each saying what the section is, who can reach it, and whether
  it is live or still in progress.
- ✅ A footer with the owner's email and GitHub.
- ✅ Sign-in walls show before the click: tiles and palette commands for
  signed-in sections say so, and following one always lands on a proper login
  page, which links back to the hub. Signing out lands there the same way.
- ✅ The site header is shared from here: the wordmark, a nav that lights the
  section you are in, and the palette. Every other page wears it too.

## Art portfolio — `/artportfolio`

The public showcase of drawings.

- ✅ Feed of posts grouped by month, with infinite "load more" paging.
- ✅ Filter rail: free-text search, tags, and curated collections — combinable,
  with live counts.
- ✅ Per-post permalinks and a JSON API of public posts.
- ✅ Visibility per post: public (listed), unlisted (permalink only), hidden.
- ✅ Admin dashboard (`/admin`, for the owner and accounts granted admin): upload with automatic image
  variants, edit captions/tags, manage collections and visibility.
- 💭 Admin redesign — `/admin` as a dark sidebar shell with live counts, a
  searchable post list, and the owner-only accounts pane folded in. Packs 3–5
  of `docs/manifests/2026-09-08-hub-admin-cv-docs.md`, not started.

## Drawing tasks — `/tasks`

- ✅ A LeetCode-style practice board for drawing: reference images carrying any
  number of task prompts, filterable by subject, difficulty, and task type.
  Public to browse; admins manage images and tasks in place.

## Fitness — `/fitness` (private, multi-user)

A nutrition and weight tracker for several people, session-gated, dark-themed.
Each person has their own log; the food catalog is shared.

- ✅ Today view: calorie/macro targets ring, week strip, meals by slot
  (breakfast/lunch/dinner/snack/other).
- ✅ Week view: calorie bars, protein stats, logging streak, weight trend,
  most-logged foods.
- ✅ Food database with favourites, custom portions, and recipes; quick-log,
  copy-day, and barcode scanning that matches known products or prefills
  new ones from OpenFoodFacts.
- ✅ Accounts: the owner signs in with a passkey; everyone else with a name and
  a PIN, created for them at `/admin/users`. There is no public sign-up.
  Entries, weights, targets and recipes are private to each person; the food
  database, favourites and portions aside, is common.
- ✅ Art-portfolio admin is a permission the owner grants and revokes per
  account — a fitness account reaches `/fitness` and nothing else. Only the
  owner manages accounts, and the owner cannot be demoted or deleted.
- ✅ Everyone manages their own name and PIN at `/fitness/account`.
- ✅ The Today screen logs food in one tap. Each logged row carries its own
  amount controls — fractions of whatever that food comes in, so half a pack
  is a tap rather than a sum — and re-logging, saved meals and "usual at this
  meal" are each a single tap. Typing a food nobody has entered yet creates it
  and logs it in the same tap, macros fillable later.
- ✅ The day says where it stands without arithmetic: calories left, a macro
  composition pie, what is still owed against each target, and a streak.
- ✅ On a phone it is one column with Scan, Search and copy-yesterday in thumb
  reach; `/` jumps to the search field and `Esc` backs out one layer.

## Sorting & loading — `/sorting` (private, per-user)

The warehouse half of a delivery round: sort the crates, then load the van.
Built for a tablet on the floor, signed in, with each person's sessions their
own.

- ✅ A generated plan is pasted in as JSON; the app parses it, then re-derives its
  arithmetic independently and shows where the plan disagrees with itself
  rather than trusting the numbers it was handed.
- ✅ The loading board is drawn from the van's own rear-right corner, so what is
  on screen matches what the driver is looking at.
- ✅ Every move is optimistic and appends to a log, so the board survives a
  reload, queues up when there is no signal on the floor, and can be undone a
  step at a time.
- ✅ The van's shape is remembered per person rather than per session — a van does
  not change between mornings — and can be reset to the plan's own defaults.
- ✅ A full-screen mode hides the site chrome for the duration of a load.

## Drinks — `/drinks`

A party platform for phone-based drinking games in shared rooms.

- ✅ Rooms joined by name + PIN; three-tab phone shell (game / standings /
  room) plus a spectator "big screen" view with live standings.
- ✅ Four games per room: Ring of Fire, 3 Man, Last Call, and Backroom.
  Last Call's beats advance when every player taps READY (no clock — the
  table sets its own pace); card swaps are free in the lobby, once a round
  after; staging and locking happen during the open Diplomacy talk beat.
- ✅ Backroom is a hidden-role game for five to ten players — you are dealt a
  secret allegiance and spend the game reading the table. It pours no
  drinks in its first version, deliberately: the room's live feed is
  readable by anyone holding the room code, including the spectator
  screen, so a drink that depended on a secret would give the secret away.
- ✅ Test play mode (`DRINKS_TEST_MODE=1`): spawn fake players and hop
  between identities to drive every seat from one browser. Off — routes
  404 — unless the server opts in, so production can never expose it.
- ✅ Installable as a home-screen app (standalone, no browser bar) with an
  in-game FULL SCREEN toggle on Android browsers.
- ✅ Player accounts with self-service rename; per-viewer personalized UI;
  optional sound effects (server drop-in).
- ✅ Mobile play flow (2026-08-13 redesign): HAND is a reading wheel — tap
  the focused card for a full-rules inspect sheet whose PLAY button jumps
  to the TABLE; TABLE is the play surface — drag or tap a tray card onto
  a target, armed plays queue on the felt with dotted arrows until LOCK
  IN; card swaps happen in a full-screen mulligan overlay; seat chips
  show everyone's per-deck hand counts; a mode badge + pull count ride
  the tab row. Container: `docs/manifests/2026-08-13-lc-mobile-play-flow.md`
- ✅ Challenge-card engine — a played challenge pauses the round for a table
  vote that settles it. No challenge card is in the deck yet, so no live game
  draws one.
- 💭 Challenge cards themselves — duels judged by table vote, solo dares, social
  penalties, and their on-screen spectacle. Container
  `docs/manifests/2026-08-14-lc-challenge-cards.md`, idle.

## CV — `/cv`

A public CV, in Norwegian, for anyone considering hiring the owner.

- ✅ Five sections: about, projects built personally, how the work gets done with
  AI assistance, work experience, and education. A skills rail sits alongside.
- ✅ Contact is email and GitHub only — deliberately no phone number and no
  postcode, since the page is public and indexable. A test enforces that, and
  also that none of the source document's unfilled blanks can ever reach it.
- ✅ Print the page for a PDF; there is no separate file to keep in sync.

## How it works — `/docs`

An employer-facing explainer of how the site is built and run, public and in
English, reached from the nav, the hub footer and the command palette.

- ✅ What the site is, the path a request takes, the six sections and who can
  use each, how an upload works, who can see what, and how its owner builds
  it with AI.
- ✅ How it stays working (the tests and the two gates), the decisions behind it
  and what it costs, and what is still to change.
- ✅ The figures the code can count — sections and migrations — are checked
  against the code by tests; the test counts carry the date they were measured.
- ✅ A README for the public repository: what the site is, how to run it
  locally, and how AI is used to build it.

## Infrastructure

- ✅ Single Rust/Axum binary, server-side rendered, SQLite storage, S3-compatible
  object storage for images. The owner signs in with a passkey (WebAuthn),
  everyone else with a name and PIN.
- ✅ Deployed to a Hetzner server via GitHub Actions on every push to `master`,
  behind nginx.
- ✅ Quality gates: `scripts/check.sh` (item) and `scripts/verify.sh` (pack).

## Considered

- 💭 Spotify song sharing — friends share tracks and queue them straight into
  their own Spotify. Parked idea: `docs/manifests/2026-10-04-spotify-song-share.md`
- 💭 Public polish — page titles with the owner's name, link previews and a
  favicon, robots.txt, and a 404 page that keeps the site header. From the
  portfolio review: `docs/handoffs/2026-09-27-portfolio-review.md` (N2).
- 💭 Public demos — something a signed-out visitor can see working: a
  read-only sorting board with the loading method explained, a drinks room
  anyone can watch, and a fitness demo account. Same handoff (N1).
- 💭 How it works, the long read — engineering notes with real code excerpts, a
  dated build log, and the loading method in plain language, under `/docs`.
  Same handoff (C10).
- 💭 Deploy and server hardening — CI building against the offline query cache,
  a database backup before every deploy, a deploy user with a pinned host key,
  and security headers. Same handoff (N3).
- 💭 Lighter feed images and a lighter stylesheet — feed thumbnails and a
  smaller `style.css`. Backlog container:
  `docs/manifests/2026-08-17-web-performance.md` (handoff W5, W7).

---

*In transit: the Hub/Admin/CV/How-it-works container —*
*`docs/manifests/2026-09-08-hub-admin-cv-docs.md`. The shared shell, the Hub,*
*its follow-up, the CV and the How-it-works page have shipped; the Admin*
*redesign is considered, not started.*
