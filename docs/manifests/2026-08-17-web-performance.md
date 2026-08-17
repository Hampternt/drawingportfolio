# Container — Web performance

**Status:** NOT STARTED — backlog, no pack in flight. Pack 2 is already landed;
packs 1 (partly) and 3 are what remains.
**Scope:** container (3 packs)
**Goal:** the site loads faster on the measures that matter to a visitor —
transfer size, largest-contentful-paint, layout stability, and perceived
latency on the one interaction people repeat.

Identified in a performance discussion on 2026-04-11 and carried since. Fast
upload (canvas WebP + background AVIF via `tokio::spawn`) shipped that same
session and is not part of this container.

Packs are ordered by cost, not by impact: pack 1 needs no rebuild and no
deploy, pack 2 rebuilds the binary, pack 3 adds a dependency. Only the active
pack gets an item list — these three are all dormant.

---

## Pack 1 — nginx only, no Rust rebuild

Config-file edits on the server, applied by hand. See the `deploying` skill
before touching anything server-side, and `deploy/nginx.conf`'s own comments.

**Already landed 2026-08-16**, when the live config was finally brought up to
date — it had still been the 2026-07-28 file, so nothing added since had ever
reached the server. Verified against the running site:

- HTTP/2 — live (`listen 443 ssl http2`, both address families).
- gzip — live; `content-encoding: gzip` + `vary: Accept-Encoding` confirmed on
  HTML. It sits in `server` context, not `http`, because Ubuntu's stock
  `nginx.conf` already sets `gzip on;` and a second one there is a hard
  `[emerg] duplicate`, not an override.
- Static caching — live; `/static/` returns
  `max-age=31536000, public, immutable`.

Still open:

- **Brotli** — roughly 15–25% better than gzip on text. Check the module first
  (`nginx -V 2>&1 | grep brotli`); on Ubuntu/Hetzner that means
  `apt install libnginx-mod-brotli`. Config: `brotli on;`
  `brotli_comp_level 6;` `brotli_types text/css application/javascript;`
- **Cache-Control on HTML responses** — distinct from the `/static/` win above.
  A short `max-age=60, stale-while-revalidate=300` helps repeat visitors.
  Needs care: admin and API routes must not be cached at all, and `/fitness` is
  multi-user, so any HTML caching there must be `private`.

## Pack 2 — Rust + CSS — LANDED, nothing to do

Both items here were **already built** by the time this backlog was migrated.
Verified against `dev` on 2026-08-17 rather than trusted:

- **LCP image priority — done.** `src/routes/feed.rs` drives "which single
  image gets `fetchpriority="high"`" off `is_first_page`, and its tests assert
  it: the first card carries `loading="eager"` + `fetchpriority="high"`, the
  rest `loading="lazy"` with no `fetchpriority`.
- **CLS from unsized images — done, by a better route than planned.** The
  backlog proposed a CSS `aspect-ratio: 4/3` box with `object-fit: cover`.
  What shipped instead is real intrinsic dimensions: migration 012 added
  `posts.image_width`/`image_height` and `templates/artportfolio/post.html`
  emits `width`/`height` when both are positive. That reserves each image's
  true box rather than forcing every drawing into 4:3, so the CSS hack should
  **not** be layered on top of it.

Kept as a record of why this pack is closed — the old backlog would otherwise
have a future session re-proposing both.

## Pack 3 — HTMX preload

- **Preload the "Load more" button.** Add `htmx-ext-preload.js` to `static/`,
  reference it from `base.html` *and* `admin.html`, then put
  `hx-ext="preload" preload="mousedown"` on the Load More button in
  `templates/artportfolio/feed.html`. The paginated request goes out on
  mousedown so the response is in flight before the click releases. Near-free
  perceived-speed win, but it is a new dependency for one control — worth doing
  last, or not at all if packs 1–2 already feel fast.

---

## Ledger

Nothing started. When a pack begins, it gets its own manifest under
`docs/manifests/` and this file points at it.

Moved here 2026-08-17 from a Claude memory file (`perf_tiers.md`), which had
been acting as a private backlog for this work. A backlog belongs in the repo
where it is reviewable and where anyone can see it; the memory now points here.
