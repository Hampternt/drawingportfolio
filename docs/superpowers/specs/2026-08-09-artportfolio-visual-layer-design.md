# /artportfolio visual layer — design spec

Date: 2026-08-09
Status: pending user approval
Slice: 1 of 5 (see "Decomposition" below)

Source of truth for visuals: `docs/design/artportfolio-redesign/README.md`.
That document states its own precedence rule — *"Where this document and the HTML
disagree, this document wins"* — so the `.dc.html` prototypes and the `_ds/` CSS are
reference, not authority. Two places where they disagree are resolved in
"Handoff corrections" below.

The design system itself is committed at
`docs/design/artportfolio-redesign/_ds/hampter-design-system-.../` (fonts excluded —
they are byte-identical duplicates of `drinkinggame/assets/fonts/`).

Opening `ArtPortfolioFeed.dc.html` in a browser renders it in fallback faces: the file
declares `font-family:'Archivo',system-ui,sans-serif` but ships no `@font-face` rule and
never references `assets/fonts/`, so the woff2 files would not change its rendering
whether present or not. Type is therefore specified by the README's tables, not by what
the preview displays.

## Goal

Render `/artportfolio` in the Hampter Design System. After this slice, `/`, `/tasks`,
`/fitness`, `/admin` and `/drinks` are visually **byte-identical** to before.

Non-goals for this slice: the visibility model, collections, tags, the multi-upload
tray, select mode, batch actions, the `?` shortcuts overlay, and any migration of
other sections to the design system.

## Decomposition

The handoff describes five slices' worth of work. This spec covers slice E only,
sequenced first so later slices write design-system-native CSS once rather than
styling twice.

| Slice | Scope | Depends on |
|---|---|---|
| **1 (this spec)** | Design-system visual layer, month grouping, caption search | — |
| 2 | Visibility model: public / unlisted / hidden | — |
| 3 | Collections + tags + full filter rail | — |
| 4 | Multi-upload tray | 2 |
| 5 | Select mode + batch actions | 2, 3 |

### Open questions for later slices

Recorded here so they are not rediscovered mid-implementation. Neither affects slice 1.

**Slice 2 — `unlisted` has no delivery mechanism.** The handoff defines unlisted as
"reachable only by direct id", but `feed.rs` exposes exactly three routes and none is a
single-post permalink. Without one, unlisted behaves identically to hidden and the
three-state model collapses to two. Slice 2 must either add
`GET /artportfolio/{id}` or redefine unlisted as "excluded from the feed, still
returned by the API".

**Slice 5 — `batch_delete` must return image URLs.** The existing `delete_post` returns
the image URL so the route can delete the object from storage. A batch delete without
the same contract orphans three objects per post (JPEG + WebP + AVIF variants). Give it
a `Vec<String>` return and clean up in the route.

## Decided rules (user-confirmed)

| Question | Decision |
|---|---|
| Slice order | Visual layer first; functional slices build on it |
| Blast radius | Everything scoped to `body.art-page`; other sections untouched |
| Slice 1 contents | Visitor view minus taxonomy; rail ships search + keyboard legend only |
| Mono face | IBM Plex Mono, self-hosted (no Google Fonts request) |
| Wordmark | `hampter.` with violet full stop, scoped to `.art-page` |
| Keyboard | `/` and `J`/`K` in this slice; `?` overlay deferred to its own slice |
| Intro copy | Keep the handoff line: *"Mostly practice — studies, drills, and the odd finished piece."* |
| Image dimensions | Migration 012 adds `image_width`/`image_height`; masonry needs them |

## Handoff corrections

Three points where the handoff is internally inconsistent or would break the repo.
These resolutions are binding.

### 1. Month dividers vs. CSS-column masonry

The handoff asks for a 3-column masonry (`column-gap: 16px`, per-card
`margin-bottom: 16px` — i.e. CSS multi-column) **and** full-width month dividers with
"a 1px rule filling the row". These do not compose: in CSS columns, content flows down
column 1 before column 2, so an inline divider would span one column's width and the
newest-first chronology would read vertically per column.

**Resolution.** Each month is its own `<section>`: a full-width divider followed by a
`columns: 3` block containing only that month's cards. Dividers stay full-bleed;
chronology reads per month. Within a single month, column-major ordering is accepted.

### 2. The design system's `styles.css` cannot be linked as-is

Two of its nine `@import`s violate the `.art-page` containment decision:

- `tokens/base.css` sets **unscoped** `body { background: var(--surface-page) }` plus
  global `a`, `p`, `h1`–`h4` and `:focus-visible` rules. Linking it turns every page
  on the site dark immediately.
- `tokens/legacy-nocturne.css` deliberately repoints every `--noc-*` variable at the
  new tokens, shifting the fitness tracker's accent from `#9184d9` to `#B48EF7`.

**Resolution.** Lift `colors`, `typography`, `spacing`, `shape` and `motion` as-is —
they contain only `:root` custom-property declarations, which are inert and affect
nothing that does not read them. Rewrite `base.css`'s resets under `body.art-page`.
**Exclude `legacy-nocturne.css` entirely** — migrating the fitness tracker is a
separate future slice, not a side effect of this one.

### 3. `object-fit: cover` on post media

`components/components.css:201` sets `.hm-post__media img { object-fit: cover }`,
which crops. The README mandates the drawing renders "uncropped at its natural ratio
(`width:100%`, no forced aspect)", and the design system's own imagery rule says
drawings are "never force-cropped to a tile".

**Resolution.** README wins. `width: 100%; height: auto; display: block`, no
`object-fit`.

### Deferred from the handoff

`position INTEGER NOT NULL DEFAULT 0` (handoff migration 012) is **not** implemented.
Upload order would write it and nothing would read it — the design only ever sorts
newest-first. It returns if manual reordering is ever specified.

## Architecture

### Scoping mechanism

`feed.html` declares `{% block body_class %}art-page{% endblock %}`.

`hx-boost` swaps body *children*, not body *attributes*, so that class is lost on
boosted navigation — the same bug the inline script at `base.html:18–31` already
solves for `fitness-dark`. That script generalises to derive both classes from a
marker element in the incoming content:

```js
function syncBodyTheme() {
  document.body.classList.toggle('fitness-dark',
    !!document.querySelector('main .fitness-page'));
  document.body.classList.toggle('art-page',
    !!document.querySelector('main .art-feed'));
}
```

bound to `htmx:afterSwap` (when `e.target === document.body`) and
`htmx:historyRestore`, exactly as today.

**The marker element is the feed wrapper.** `feed.html`'s outermost element inside
`{% block content %}` is `<div class="art-feed">`, wrapping the page head, rail and
grid. `syncBodyTheme` keys off that and nothing else — naming it here so the
boost-navigation behaviour is not left to the implementer's choice.

`templates/admin.html` carries its own copy of this script and receives the identical
edit — required by the CLAUDE.md rule that global features land in both shells.

Every rule that paints something is written under `body.art-page`. Token declarations
land on bare `:root`.

### Server changes

**`src/db.rs`**

| Function | Behaviour |
|---|---|
| `get_posts_page(pool, q: Option<&str>, page: i64) -> Vec<Post>` | Renames `get_posts` and adds the `q` parameter. Keeps the N+1 fetch so `has_more` needs no COUNT. When `q` is `Some`, adds `WHERE caption LIKE ?1 ESCAPE '\'`. |
| `count_posts(pool, q: Option<&str>) -> i64` | One COUNT for the page head's `117 drawings`. Called only on full page render, never on HTMX pagination. |

**Forward note for slice 2:** `count_posts` is correct today only because every post is
visible. When the visibility model lands it must take `is_admin` and count
`visibility = 'public'` for visitors, or the page head will overcount. The same applies
to `get_posts_page`. Both are listed in slice 2's scope for that reason.

The `LIKE` pattern is built in Rust: escape `\`, `%` and `_` in the user's string
(in that order), then wrap in `%…%`. Without this, a caption search for `100%` matches
every row.

**`get_posts` has three call sites, not one.** `feed.rs` uses it twice, `admin.rs:52`
uses it to build the admin dashboard, and `db.rs`'s own tests call it. All are updated
to `get_posts_page(pool, None, page)`; the admin dashboard's behaviour is unchanged.

Migration 012:

```sql
ALTER TABLE posts ADD COLUMN image_width  INTEGER NOT NULL DEFAULT 0;
ALTER TABLE posts ADD COLUMN image_height INTEGER NOT NULL DEFAULT 0;
```

Added to `run_migrations()` with the existing `let _ =` duplicate-column tolerance.
Existing rows keep `0` and simply render without dimension attributes — no backfill.

Note the numbering diverges from the handoff, which reserved 012 for visibility.
Because the visual layer ships first, **012 is image dimensions** and the visibility
migration becomes 013 in slice 2. Migration numbers follow ship order, not the
handoff's listing order.

Schema changes require the sqlx offline-cache ritual from CLAUDE.md: apply to the
local dev DB, run `DATABASE_URL=sqlite:portfolio.db cargo sqlx prepare`, commit
`.sqlx/`. This slice touches the `drawingportfolio` crate, which *does* use the
offline cache — unlike `drinkinggame`.

**Regenerate the whole cache, not just the new query.** Six of the 60 `.sqlx` entries
reference `posts`; adding two columns changes the row shape every one of them
describes, and `get_posts_page` is a renamed function with a new SQL string on top of
that. `cargo sqlx prepare` rewrites the directory wholesale, so this is automatic as
long as it is actually run. `scripts/verify.sh:44` runs `cargo test` under
`SQLX_OFFLINE=true`, so a stale cache fails the gate rather than the deploy — but the
error surfaces as a compile failure in an unrelated query, which is confusing if the
ritual was skipped.

`Post` derives `Serialize` and is returned by `GET /artportfolio/api/posts`, so the two
new fields become part of that endpoint's public JSON. This is an accepted additive
change, not an oversight — the API is unversioned and single-consumer.

**`src/models.rs`** — `Post` gains `image_width: i64`, `image_height: i64`. New
`MonthGroup { label: String, count: usize, posts: Vec<Post> }`.

**`src/routes/admin.rs`** — the upload handler already decodes the image with the
`image` crate to generate the WebP variant, so `.dimensions()` is free at that point.
Those values are written on insert. Nothing else in this file changes; in particular
`admin_post_card_html()` and the three-layer 35 MB limit are untouched.

**`src/routes/feed.rs`**

- `PageQuery` gains `q: Option<String>` and `last_month: Option<String>`.
- `post_card_html()` and `render_posts_html()` are deleted; both callers render Askama
  templates instead. `html_escape()` stays — `admin.rs` still uses it.
- Month grouping happens in the handler, producing `Vec<MonthGroup>` keyed on
  `created_at[..7]` (ISO8601 `TEXT`, so the first seven characters are `YYYY-MM`).
  Templates receive the finished groups and contain no logic.
- The inline-first-page optimisation is preserved: `feed_page` still renders page 0
  into the HTML so no second round trip is needed.
- `api_posts` gains the same `q` filter, so the JSON API and the HTML feed cannot drift.

**Why `last_month` exists.** If page 0 ends mid-June and page 1 opens with more June
posts, appending would render a second `2026-06` divider. The Load more URL carries
the last month label it rendered; the handler suppresses a leading divider that
matches it.

### Templates

`templates/artportfolio/`:

| File | Renders |
|---|---|
| `feed.html` | Shell: extends `base.html`, sets `body_class`, page head, rail + main flex, `#feed` |
| `partials/filter_rail.html` | Search field, keyboard legend |
| `partials/post_grid.html` | Month sections + Load more — the HTMX swap target |
| `partials/empty_state.html` | `>` prompt line |

`templates/partials/post_card.html` is **overwritten**. It currently exists but is
orphaned — no `#[derive(Template)]` in the workspace references it, and every card
today is built by a format string. It becomes the single real card template, rendered
via `#[derive(Template)] struct PostCardTemplate<'a> { post: &'a Post, is_first: bool }`
and used by both the inline first page and the HTMX route, preserving the
no-drift property the current shared helper provides.

Askama auto-escapes, so the card partial drops the manual `html_escape` calls.

The card keeps the existing `loading="eager" fetchpriority="high"` treatment for the
first card and `loading="lazy"` for the rest, and keeps the `<picture>` element with
AVIF and WebP sources falling back to the original — including the existing
empty-URL-means-variant-failed behaviour.

`width`/`height` attributes are emitted only when both dimensions are non-zero.

### Page head

```
micro-label   11px IBM Plex Mono, uppercase, .10em, --text-faint
              "117 drawings · newest first"
h1            Archivo 800 40px/1.22, −0.025em, --text-strong
              "Drawing Portfolio"
intro         Space Grotesk 400 15px/1.5, --text-muted, max 68ch
              "Mostly practice — studies, drills, and the odd finished piece."
background    --surface-page + 32px blueprint grid at 4.5% white
```

Under an active search the micro-label reads `12 drawings · matching "loomis"`.

### Static assets

**`static/style.css`** — one new section, `/* ── Drawing Portfolio ─────── */`,
appended at the end. Contains, in order: the token `:root` blocks lifted verbatim from
`colors/typography/spacing/shape/motion.css`; the `@font-face` block; the `base.css`
resets rewritten under `body.art-page`; and the component subset —
`hm-icon`, `hm-btn` (`--secondary`, `--ghost`, `--sm`, `--md`), `hm-iconbtn`,
`hm-kbd` + `hm-kbd-group`, `hm-card`, `hm-post*`, `hm-input*`, `hm-field*`,
`hm-grid-bg` — plus page-specific layout rules. `Tag` and `Badge` are not lifted;
they belong to slices 3 and 2.

CLAUDE.md forbids nested `/* */` markers here — `tests/static_assets.rs` guards it.

**The existing `.post-card`, `.load-more` and `.empty-state` rules stay.** The new card
partial emits `hm-post` markup, so those selectors stop matching anything on
`/artportfolio` — but `admin.rs`'s `admin_post_card_html()` still emits
`class="post-card"` for the admin dashboard. Deleting them would break `/admin`. They
are removed in whichever slice migrates the admin dashboard, not this one.

**`static/fonts/`** — seven woff2:

- `archivo-700/800/900.woff2`, `space-grotesk-400/500.woff2` copied from
  `drinkinggame/assets/fonts/` (the design system's copies are byte-identical).
- `ibm-plex-mono-400/500.woff2` fetched from Google Fonts and self-hosted, matching
  how `base.html` self-hosts HTMX to avoid a third-party handshake.

`archivo-800.woff2` and `space-grotesk-400.woff2` get `<link rel="preload">` in the
`{% block head %}` of `feed.html` — not in `base.html`, which would make every other
page pay for fonts it does not use.

`deploy.yml:65` already rsyncs `static/`, so no CI change is needed.

**`static/icons/`** — three Lucide v0.462.0 SVGs (`search.svg`, `chevron-down.svg`,
`x.svg`), self-hosted and applied as CSS masks over `currentColor` per `.hm-icon`.
The visitor view needs no others; later slices add theirs.

**`static/artfeed.js`** — new, ~60 lines:

- `/` focuses the search field; `Esc` blurs it.
- `J` / `K` step focus between cards with `scrollIntoView({block:'nearest'})`.
- All single-letter handling is suppressed when `document.activeElement` is an
  `input`, `textarea` or `select`, per the design system's reserved-key rules.
- Binds on both `DOMContentLoaded` and `htmx:afterSwap`, guarded against
  double-injection — the pattern `static/palette.js` already uses.
- Respects `prefers-reduced-motion` by using `behavior: 'auto'` for scrolling.

**`static/palette.js`** — unchanged. "Go to Art Portfolio" already exists; the upload
and filter commands belong to slices 4 and 3.

### HTMX contract

| Interaction | Contract |
|---|---|
| Search | `hx-get="/artportfolio/htmx/posts"`, `hx-trigger="keyup changed delay:200ms"`, `hx-target="#feed"`, `hx-swap="innerHTML"`, `hx-push-url="true"` |
| Load more | `hx-get="/artportfolio/htmx/posts?page=N&q=…&last_month=YYYY-MM"`, `hx-target="#load-more"`, `hx-swap="outerHTML"` — unchanged from today apart from the extra params |

`hx-push-url` on search means a filtered feed is linkable and survives reload;
`feed_page` reads the same `q` off the query string.

### Responsive

Breakpoints are disjoint — the handoff's "masonry 2 columns 780–1179" and "under 900px
the rail collapses" overlap, so the 780–899 band is spelled out explicitly:

| Width | Rail | Masonry |
|---|---|---|
| ≥ 1180px | 236px, sticky at `top: 80px` | 3 columns |
| 900–1179px | 236px, sticky | 2 columns |
| 780–899px | Collapsed above the feed | 2 columns |
| < 780px | Collapsed above the feed | 1 column |

Collapsed means the rail becomes a full-width search field above the grid. The
handoff's full-screen filter sheet belongs to slice 3 — with only a search field to
hold, a sheet would be an empty gesture.

### Motion and focus

130ms controls, 190ms surfaces, `cubic-bezier(.2,.8,.3,1)`. Press is
`translateY(1px)`, never a scale. Card hover lifts 2px and adds `--shadow-2`.
Focus is the design system's single double ring — 2px page colour, then 2px violet —
applied via `:focus-visible` scoped to `body.art-page`.
`@media (prefers-reduced-motion: reduce)` zeroes every duration token.

## Testing

`./scripts/verify.sh` is the acceptance gate for every task. It runs `cargo fmt
--check`, `cargo clippy`, the workspace suite, and `node --check` over `static/*.js`
— which covers the new `artfeed.js`.

New tests:

| Test | Asserts |
|---|---|
| `get_posts_page` unfiltered | Same results as the old `get_posts` — pagination and `has_more` unchanged |
| `get_posts_page` filtered | Matches captions case-insensitively; non-matching rows excluded |
| LIKE escaping | A caption containing a literal `%` does not make the search match every row |
| `count_posts` | Agrees with the filtered result count |
| Month grouping | Posts spanning three months produce three groups in newest-first order with correct counts |
| `last_month` suppression | A page whose first group equals `last_month` renders no leading divider |
| Card render | Escapes `<script>` in captions; omits `width`/`height` when dimensions are `0`; emits them when non-zero |
| Migration idempotence | `run_migrations()` twice on one pool succeeds |

`tests/static_assets.rs` already guards the new CSS section against nested comment
markers.

Manual verification before completion, per the user-global rule that UI changes are
opened in a real browser: load `/artportfolio` at 1280px, 900px and 390px; confirm
`/`, `/tasks` and `/fitness` are unchanged; navigate between sections via `hx-boost`
and confirm the `art-page` class attaches and detaches correctly.

## Risks

| Risk | Mitigation |
|---|---|
| A DS rule leaks past `body.art-page` and restyles another section | Every painting rule is prefixed; manual check of `/`, `/tasks`, `/fitness` is an explicit acceptance step |
| Askama's escaping differs from the hand-rolled `html_escape`, changing output | Card render test asserts the escaped output directly |
| Masonry reflow on image load | Migration 012 dimensions; legacy rows without them are the only ones that shift |
| Font files inflate first paint | Five of seven faces are already on disk; only the two Plex files are new bytes, and only two faces preload |
| `image` crate `.dimensions()` unavailable at the point of insert | It is on the already-decoded `DynamicImage` used for the WebP variant; if the decode path changes, the columns default to `0` and cards degrade to today's behaviour |
