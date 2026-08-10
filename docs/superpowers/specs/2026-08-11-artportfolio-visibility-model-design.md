# /artportfolio visibility model — design spec

Date: 2026-08-11
Status: pending user approval
Slice: 2 of 5 (decomposition in `2026-08-09-artportfolio-visual-layer-design.md`)

Source of truth for visuals: `docs/design/artportfolio-redesign/README.md`, sections
S2 and "Interactions & behaviour". Slice 1 shipped the design-system layer, so this
slice writes design-system-native markup directly and adds no new visual vocabulary
beyond one badge.

Slice 1's spec left this slice one open question — *"`unlisted` has no delivery
mechanism"* — and two forward notes, on `count_posts` and `get_posts_page` needing the
visibility filter. All three are resolved below.

## Goal

Give every post one of three states — `public`, `unlisted`, `hidden` — enforce that
state on every route that reads posts, and let an admin change it inline from the feed.

After this slice a visitor can reach an unlisted post only by its permalink, and can
reach a hidden post not at all.

Non-goals: collections, tags, the rail's filter groups (slice 3), the multi-upload
tray (slice 4), select mode and batch visibility (slice 5), and post reordering — the
handoff's `position` column is not added here, because nothing reorders yet.

**Also a non-goal, and worth stating so it is not filed as a bug:** the `/admin`
dashboard shows no visibility badge. It renders through `admin_post_card_html()`, the
legacy format string, which slice 1 deliberately left alone and which a later slice
migrates. So an admin who hides a post will see it unchanged on `/admin` — correctly
listed, just unlabelled. The feed is where state is visible in this slice.

## The three states

| visibility | in feed | in JSON API | permalink, visitor | permalink, admin |
|---|---|---|---|---|
| `public`   | yes | yes | 200 | 200 |
| `unlisted` | no  | no  | 200 | 200 |
| `hidden`   | no  | no  | **404** | 200 |

`hidden` returns 404 rather than 403 deliberately: a 403 confirms the row exists, which
is the one fact hiding it is meant to withhold.

### Unlisted is "not listed", not "secret"

The permalink is `GET /artportfolio/{id}` and post ids are sequential, so anyone can
walk `/artportfolio/1..N` and find every unlisted post. This is an accepted trade-off,
not an oversight. Unlisted means *"excluded from my feed and from the API"* — enough to
keep a work-in-progress off the front page and still send someone a link to it.

If it ever needs to mean secret, the upgrade is additive and does not disturb the state
model: add `posts.share_token TEXT` (random base62, backfilled), serve
`GET /artportfolio/p/{token}`, and stop putting ids in URLs. Explicitly not built now.

Two alternatives were rejected. **Unlisted-in-the-API-only** (no permalink) leaves the
raw JSON endpoint as the only shareable artifact and makes the API a deliberate privacy
hole. **Two states, public and hidden** is smaller but discards the third state the
handoff, the slice-1 spec and the rail design all assume.

## Server changes

### Migration 013

```sql
ALTER TABLE posts ADD COLUMN visibility TEXT NOT NULL DEFAULT 'public';
```

Added to `run_migrations()` with the existing `let _ =` duplicate-column tolerance.
Every existing row becomes `public`, which is what it already was in effect — no
backfill, no behaviour change for anyone who does not use the new controls.

**No `posts(visibility, created_at DESC)` index**, though the handoff lists one. See
"SQL shape" — the query shape this spec settles on cannot use it, and at the current
row count the scan is free. Adding a dead index is worse than adding none.

The number is 013, not the handoff's 012: slice 1 shipped image dimensions as 012.
Migration numbers follow ship order.

### `src/models.rs`

```rust
/// The three states a post can be in.
pub enum Visibility { Public, Unlisted, Hidden }   // as_str / from_str

/// Who is asking. Built at the handler edge from OptionalAuth.
pub enum Viewer { Visitor, Admin }
```

`Post` gains `visibility: String` — the column's raw text, parsed to `Visibility` where
behaviour depends on it. Storing the enum on `Post` directly would need a sqlx type
override on every one of the six queries that select `posts`; the parse is cheap and
happens in one place.

`from_str` returns `Option`. Two different places consume that `None`, and they must not
be confused:

- **Reading a row from the DB** — a corrupt or future value is treated as `Hidden`. Fail
  closed; a value nobody recognises must not render to the public.
- **Accepting the `PATCH` body** — a `None` is a `400`. Fail loudly; silently coercing a
  typo'd state to `Hidden` would look like the request succeeded.

`Post` is `Serialize`, so adding the field also adds `visibility` to every object the
JSON API returns. That is intentional and harmless: the API serves only `public` rows to
a visitor, so the field always reads `"public"` there, and an admin gets the real value.

### `src/db.rs` — the `Viewer` parameter

Every function that reads posts takes `viewer: Viewer`. It is a required parameter with
no default, so omitting it is a compile error rather than a silent leak.

| Function | Change |
|---|---|
| `get_posts_page(pool, q, page, viewer)` | new parameter; visitors see `public` only |
| `count_posts(pool, q, viewer) -> PostCounts` | returns the split counts, see below |
| `get_post_by_id(pool, id, viewer) -> Option<Post>` | new; `None` for a hidden post seen by a visitor |
| `set_post_visibility(pool, id, visibility)` | new |

**The plan must name the viewer value at every call site.** "Required parameter =
compile error" only protects against forgetting, not against satisfying the compiler
with the wrong value:

| Call site | Viewer |
|---|---|
| `feed.rs::feed_page` | from `OptionalAuth`, then the preview override |
| `feed.rs::htmx_posts` | from `OptionalAuth`, then the preview override |
| `feed.rs::api_posts` | from `OptionalAuth` |
| `admin.rs:52`, the admin dashboard | **always `Viewer::Admin`** |
| `db.rs` tests | explicit per test |

`admin.rs:52` is the one that fails quietly: pass `Visitor` and the dashboard still
compiles, still renders, and simply stops showing the posts the admin most needs to see.

### SQL shape

`get_posts_page` keeps **two** branches, `q` and no-`q`, and binds the viewer as a
boolean:

```sql
WHERE (? OR visibility = 'public') AND caption LIKE ? ESCAPE '\'
```

The naive alternative is 2×2 = four near-identical SQL literals, because the sqlx macro
needs literal SQL. That was worth avoiding, but not worth *assuming* away: `db.rs:120`
records that this file already retreated from placeholder-inference trouble once, so a
parameter in boolean position needed proof rather than optimism.

**Measured, not reasoned.** Three candidate queries were added to `db.rs`, compiled
with `DATABASE_URL` pointed at a copy of `portfolio.db` carrying migration 013, then
reverted. All three infer cleanly:

```
WHERE (? OR visibility = 'public')                    bool bind    compiles
WHERE (visibility = 'public' OR ? = 1)                i64 bind     compiles
SELECT visibility AS "visibility!: String",
       COUNT(*) AS "n: i64" FROM posts GROUP BY visibility        compiles
```

The bool bind is the one to ship. If it ever regresses, the `i64` form is the fallback,
and four literal branches the last resort.

**This is why there is no index.** SQLite cannot use `posts(visibility, created_at
DESC)` against an OR-with-a-parameter predicate — it scans regardless. Had this spec
taken the four-branch route, the visitor branch would be a plain
`WHERE visibility = 'public' ORDER BY created_at DESC` and the index would earn its
place. Two branches and no index is the better trade at a few hundred rows; the two
decisions are coupled and must move together if either is revisited.

### `count_posts` returns a split

```rust
pub struct PostCounts { pub total: i64, pub public: i64, pub unlisted: i64, pub hidden: i64 }
```

One `GROUP BY visibility` query, not three COUNTs. For a visitor, `total` is the public
count and the other three fields are not rendered.

**`GROUP BY` returns no row for a state with zero posts**, so the struct is built by
defaulting every absent key to `0` — not by indexing the result. A portfolio with
nothing hidden is the normal case, not an edge one.

`head_label()` therefore takes the viewer as well, and the label has four shapes:

| | no search | searching |
|---|---|---|
| **visitor** | `117 drawings · newest first` | `12 drawings · matching "cat"` |
| **admin** | `128 drawings · 117 public · 4 unlisted · 7 hidden` | `12 drawings · matching "cat" · 9 public · 2 unlisted · 1 hidden` |

The split **replaces** the `· newest first` sort suffix and **follows** `· matching
"…"` when there is a search. Sort order is unchanged and stated by the toolbar, so
spending head room on it while withholding the counts would be the wrong trade.

Today's `count_posts` carries `as "count: i64"` because sqlx infers SQLite's `COUNT(*)`
as `i32` (`db.rs:153`). The `GROUP BY` version is `query_as!` rather than
`query_scalar!`, so that override changes form to `AS "n: i64"` and is easy to drop on
the way. The probe above confirms the working form.

### `src/routes/feed.rs`

**`htmx_posts` and `api_posts` do not currently extract `OptionalAuth`.** Only
`feed_page` does. Today that is harmless because every post is public; the moment
visibility exists it is a live leak — page 0 renders filtered, and the first *Load more*
hands back hidden posts. **This is the highest-risk change in the slice** and the reason
the tests below assert at `page >= 1`, not only page 0.

New route: `GET /artportfolio/{id}` → `templates/artportfolio/post.html`, extending
`base.html`, rendering the drawing at full width with its caption, date and a link back
to the feed. No collision with the existing routes — `/artportfolio/htmx/posts` and
`/artportfolio/api/posts` are two segments deep, the permalink is one.

`404` is returned as a `StatusCode::NOT_FOUND` with a minimal page, for both a missing
id and a hidden post seen by a visitor. The two cases are indistinguishable from
outside, which is the point.

Cards in the feed link to their permalink.

### View as visitor

`?visitor=1` on `/artportfolio` forces `Viewer::Visitor` even with a valid session, so
an admin can see what a visitor sees. The handoff gives it the `V` shortcut and a
secondary button in the head; while active, the head shows a way back.

**It must be added to `PageQuery` and threaded through `load_more_url()`, `page_url()`
and `head_label()`, exactly as `q` and `last_month` already are.** Without the first
two, page 0 renders as a visitor and the first *Load more* renders as an admin — hidden
posts appear mid-feed in the middle of the preview that exists to prove they do not.
Without the third, a search while previewing swaps in the admin-shaped head (`…· 7
hidden`) above a visitor-shaped feed — the out-of-band label at `feed.rs:239` is
computed on the page-0 path and would otherwise never learn about the preview.

The permalink route honours the same flag: a previewing admin gets the 404 on a hidden
post. A preview that quietly shows more than a visitor would is worse than no preview.

### `src/routes/admin.rs`

- `PATCH /api/admin/posts/{id}/visibility` — body carries the new state; an unrecognised
  string is a 400, not a silent no-op. Responds with the re-rendered card, swapped
  `outerHTML` into `closest .hm-post`, so the badge and opacity update without a reload.
- The upload handler gains an optional `visibility` multipart field, defaulting to
  `public`. The three-layer 35 MB limit is untouched.

## Template and style changes

`templates/partials/post_card.html` is the single source of card markup and stays that
way — it is rendered from three places, and slice 1's checkpoint recorded that missing
the third (`admin.rs:211`, the upload response) is invisible to both the compiler and
the tests.

The card gains, **admin only**:

- a visibility badge at top-right, 8px inset — `public` mint `#4FD6A8`, `unlisted` amber
  `#FFB570`, `hidden` neutral
- `opacity: .5` on a hidden card
- a hover/focus control cluster at top-left with hide and unlist actions

A visitor's card is byte-identical to slice 1's. The badge and cluster live behind
`{% if is_admin %}`, which means `PostCardTemplate` gains an `is_admin` field and all
three render sites pass it.

Styles go in `style.css` under the existing `body.art-page` section — never a `<style>`
block, per the architecture rules.

## Testing

| Test | Why it exists |
|---|---|
| visitor sees neither unlisted nor hidden, on **all three** feed routes | the leak this slice creates |
| the same assertion at `page >= 1` | page-0-only coverage passes while the real *Load more* leak ships |
| admin sees all three | the filter must not over-apply |
| `/admin` dashboard lists all three | catches `admin.rs:52` being passed `Visitor` |
| permalink: public 200 · unlisted 200 · hidden 404 visitor / 200 admin | the state table above, executable |
| `?visitor=1` hides hidden posts on page 0 **and** page 1 | the `load_more_url` threading bug |
| `PATCH` with an unknown string → 400 | fail closed |
| `Visibility::from_str` round-trip, unknown → `Hidden` | fail closed |
| head counts split correctly | `count_posts`' new shape |

## Mechanical steps that break the build if skipped

- **sqlx offline cache.** Six of the 60 `.sqlx` entries reference `posts`, and a new
  column changes the row shape every one of them describes. Apply 013 to the dev DB,
  run `DATABASE_URL=sqlite:portfolio.db cargo sqlx prepare`, commit `.sqlx/`. CI builds
  with `SQLX_OFFLINE=true` and fails without it.
- **`docs/WORKTREES.md`** still reads `Next | Write the slice 2 plan`. Update it in the
  same commit as this spec — **and land that update on `master`**, whose copy is already
  stale about slice 1's Plan B. That file exists specifically to stop orientation notes
  becoming invisible by living only on a feature branch.
- **CLAUDE.md test counts** move again. Re-measure with `cargo test --workspace`; never
  copy the number from another document.

## Sizing

Slice 1 did not fit one plan under `plan-economics` and this one probably does not
either. Candidate split, to be confirmed when the plan is written:

| Plan | Scope |
|---|---|
| **A** | migration 013 · `Visibility`/`Viewer` · db filtering · `OptionalAuth` on all three feed routes · permalink route + template · sqlx cache |
| **B** | `PATCH` route · upload field · card badge, opacity and control cluster · split head counts · view-as-visitor |

A and B are **not** safely parallel: both touch `feed.rs`, `post_card.html` and the same
`style.css` section, the same reason slice 1's two plans ran in sequence.

## Acceptance

`./scripts/verify.sh` green, plus a browser checkpoint covering: a visitor's feed at
each state, the permalink at each state, `?visitor=1` across a *Load more* boundary, and
a visibility change swapping a single card.

Note the environmental limits slice 1's checkpoints recorded and this one inherits:
`resize_window` never changes `innerWidth` here, so responsive bands are verifiable only
through the CSSOM, key events are synthetic rather than natively delivered, and Dark
Reader repaints colour — so the badge colours above need human eyes, not a screenshot.
