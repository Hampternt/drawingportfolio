# Artportfolio slice 3 — collections, tags, full filter rail

Date: 2026-08-12
Slice: 3 of 5 (decomposition in `2026-08-09-artportfolio-visual-layer-design.md`)
Design source: `docs/design/artportfolio-redesign/README.md` (rail, pills, S5
empty state) — visuals are settled there; this spec pins data model, routes and
contracts.

## Goal

`/artportfolio` gains collections and tags: a filter rail that filters
server-side by search, tags (AND), one collection, and — for admins — a
visibility subset; plus the per-card editing needed to assign any of it
(pencil: caption + tags; folder-plus: collection membership). Filters are
linkable URLs and survive reload and Load more.

## Non-goals

- Mobile full-screen filter sheet — deferred to a later slice (decision
  2026-08-12). Mobile keeps the current stacked rail.
- Sort button, select mode, batch actions — slice 5.
- Multi-upload tray — slice 4.
- Collection rename — create + delete only, rename waits until it is missed.
- `api/posts` JSON shape unchanged (no tags/collections in the payload; it
  only learns to respect the new filter params). Recorded trade-off.

## Frozen seam (slice 4 codes against this)

The OOB head-count fragment keeps its exact id and shape:

```html
<div class="hm-eyebrow art-head__label" id="art-head-label" hx-swap-oob="true">…</div>
```

`head_label()` stays the single producer. Slice 4's upload-response OOB fix
targets this id; nothing in slice 3 may rename or restructure it.

## Data model — migration `014_collections_tags.sql`

```sql
CREATE TABLE IF NOT EXISTS collections (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    slug TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE TABLE IF NOT EXISTS post_collections (
    post_id INTEGER NOT NULL REFERENCES posts(id) ON DELETE CASCADE,
    collection_id INTEGER NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
    PRIMARY KEY (post_id, collection_id)
);
CREATE TABLE IF NOT EXISTS tags (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE
);
CREATE TABLE IF NOT EXISTS post_tags (
    post_id INTEGER NOT NULL REFERENCES posts(id) ON DELETE CASCADE,
    tag_id INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    PRIMARY KEY (post_id, tag_id)
);
CREATE INDEX IF NOT EXISTS idx_post_tags_tag ON post_tags(tag_id);
CREATE INDEX IF NOT EXISTS idx_posts_visibility_created
    ON posts(visibility, created_at DESC);
```

Posts↔collections is many-to-many. Orphan tags (no posts) are left in place —
`list_tags_with_counts` naturally hides count-0 tags from visitors; a later
slice may garbage-collect. Recorded trade-off.

**Normalization (one helper, `normalize_tags`, unit-tested):** trim, lowercase,
drop empties, dedupe preserving first occurrence, max 40 chars per tag, max 20
tags per post (excess silently dropped). Collection slugs: lowercase, runs of
non-alphanumerics → `-`, trimmed of leading/trailing `-`; duplicate slug on
create → 409 with the existing collection's name in the body.

## Filtering

**URL contract** (HTMX `hx-push-url="true"`, so all of these are shareable):

```
/artportfolio?q=loomis&tags=ink,perspective&collection=studies&vis=public,hidden
```

- `tags` — comma-separated, normalized before querying, combine as **AND**.
- `collection` — one slug. Unknown slug filters to zero rows (no 404).
- `vis` — comma-separated subset of `public,unlisted,hidden`. **Admin-only:
  silently ignored for visitors** (the feed is public; erroring would leak
  that the param exists). Absent ⇒ admin sees all three.
- Load more and `htmx/posts` and `api/posts` all take the same params.

**Query shape** (decision 2026-08-12: one `query_as!` macro, null-tolerant
params, `json_each` for the two list params — compile-time checked, replaces
the existing two-variant `match` in `get_posts_page`):

```sql
SELECT id, caption, image_url, webp_url, avif_url, format, file_size_bytes,
       created_at, image_width, image_height, visibility
FROM posts WHERE
    (?1 OR visibility = 'public')
AND (?2 IS NULL OR caption LIKE ?2 ESCAPE '\')
AND (?3 IS NULL OR id IN
     (SELECT post_id FROM post_collections pc
      JOIN collections c ON c.id = pc.collection_id WHERE c.slug = ?3))
AND (?4 IS NULL OR id IN
     (SELECT post_id FROM post_tags pt
      JOIN tags t ON t.id = pt.tag_id
      WHERE t.name IN (SELECT value FROM json_each(?4))
      GROUP BY post_id
      HAVING COUNT(DISTINCT t.id) = json_array_length(?4)))
AND (?5 IS NULL OR visibility IN (SELECT value FROM json_each(?5)))
ORDER BY created_at DESC LIMIT 21 OFFSET ?6
```

`?4`/`?5` are JSON array strings (e.g. `["ink","perspective"]`) or SQL NULL.
`?1` is `viewer.is_admin()` — the existing boolean trick. `count_posts` grows
the same clauses so head counts reflect the active filter. N+1 pagination
unchanged.

**Interfaces:**

```rust
pub struct PostFilter {
    pub q: Option<String>,
    pub tags: Vec<String>,          // normalized; empty = no tag filter
    pub collection: Option<String>, // slug
    pub vis: Option<Vec<String>>,   // None = viewer default; Some = admin subset
}
pub async fn get_posts_page(pool, filter: &PostFilter, page: i64, viewer: Viewer) -> Vec<Post>;
pub async fn count_posts(pool, filter: &PostFilter, viewer: Viewer) -> PostCounts;
```

`PageQuery` grows `tags: Option<String>`, `collection: Option<String>`,
`vis: Option<String>`; a `PageQuery::filter(viewer) -> PostFilter` method owns
the parse (split, normalize, drop `vis` for non-admins) so all three read
routes share it.

## New db.rs functions

All SQL stays in `db.rs`; signatures are the task Interfaces blocks' source of
truth.

```rust
pub async fn list_collections_with_counts(pool, viewer: Viewer) -> Vec<CollectionWithCount>;
pub async fn list_tags_with_counts(pool, viewer: Viewer) -> Vec<TagWithCount>;
pub async fn create_collection(pool, name: &str) -> Result<Collection, CreateCollectionError>; // slug conflict
pub async fn delete_collection(pool, id: i64) -> bool;      // rows unlink via cascade; posts survive
pub async fn set_post_tags(pool, post_id: i64, tags: &[String]) -> bool; // replace-all semantics
pub async fn update_post_caption(pool, post_id: i64, caption: &str) -> bool;
pub async fn add_post_to_collection(pool, post_id: i64, collection_id: i64) -> bool;    // idempotent
pub async fn remove_post_from_collection(pool, post_id: i64, collection_id: i64) -> bool;
pub async fn get_post_tags(pool, post_id: i64) -> Vec<String>;
pub async fn get_post_collection_ids(pool, post_id: i64) -> Vec<i64>;
```

Counts are viewer-aware: visitors count public posts only; admins count all.
Visitors' rail hides collections and tags whose visible count is 0.

**Models (`models.rs`):** `Collection { id, name, slug, created_at }`,
`CollectionWithCount`, `TagWithCount { name, count }`.

## Routes

`feed.rs` (existing three read routes): thread `PostFilter` through page,
`htmx/posts`, `api/posts`; `head_label()` takes the filter into account the
same way it handles `q` today.

`admin.rs`, all behind `AuthSession`, registered in its `router()`:

| Route | Body (form-encoded) | Response |
| --- | --- | --- |
| `POST /api/admin/collections` | `name` | 201 + rail fragment; 409 duplicate slug |
| `DELETE /api/admin/collections/{id}` | — | 200 + rail fragment |
| `PATCH /api/admin/posts/{id}` | `caption`, `tags` (comma-sep) | 200 + re-rendered card (`closest .hm-post` outerHTML, the slice-2 pattern) |
| `POST /api/admin/posts/{id}/collections/{cid}` | — | 200 + refreshed membership checklist fragment |
| `DELETE /api/admin/posts/{id}/collections/{cid}` | — | same |

**Rail staleness, accepted:** editing a card's tags or memberships does not
OOB-update the rail counts; they self-correct on the next filter action or
page load. Single-user site; do not refile as a bug. If it grates, the fix is
an OOB rail fragment on the PATCH response — additive.

## UI

- **`filter_rail.html`** — collections list (label + right-aligned mono count,
  active row highlighted), tag pills (active = filled), keyboard legend stays.
  Admin extras: **+** IconButton beside "Collections" toggling an inline
  name input (form → `POST /api/admin/collections`), a per-row delete control,
  and the **Visibility · admin** three-checkbox group (all checked default).
  Every control is an HTMX GET to `/artportfolio/htmx/posts` with the full
  current query string, `hx-target="#feed"`, `hx-swap="innerHTML"`,
  `hx-push-url="true"`. Search keeps its 200ms debounce.
- **Active-filter row** (S5) — removable pills above the grid + ghost
  **Clear filters**; rendered by the feed page and by `htmx/posts` page 0.
- **Empty state** — extend `empty_state.html` to echo the filter set:
  `> no drawings match perspective + ink + "loomis"`, with **Reset filters**.
- **Card hover cluster** (admin) — add `pencil` and `folder-plus` IconButtons
  beside the slice-2 visibility controls. Pencil opens an in-card popover:
  caption textarea + tags input + Save (`hx-patch`). Folder-plus opens a
  checklist of collections (checkbox per collection, hits the membership
  routes). One popover open at a time; `Esc` closes — plain `artfeed.js`
  additions, bound on `DOMContentLoaded` **and** `htmx:afterSwap`, existence-
  guarded (hx-boost rule).
- **`style.css`** — everything under the existing `body.art-page` section;
  no template `<style>` blocks.
- **`palette.js`** — add "Filter drawings by tag" (navigates to the feed and
  focuses the rail), keeping the COMMANDS-array shape.

## Testing

db (`src/db.rs`): multi-tag AND (post with `ink` only excluded from
`ink+perspective`), viewer-aware counts, `set_post_tags` replace semantics,
`normalize_tags` edge cases (case, dupes, caps), delete-collection unlinks but
keeps posts, LIKE escaping combined with a tag filter, unknown slug → empty.

routes (`feed.rs` / `admin.rs`): every new mutation 401s without a session;
`vis` ignored for visitors (public-only rows come back); filters carried by
page-1 pagination; PATCH re-render contains the new caption; 409 on duplicate
collection.

One browser checkpoint at slice end (popovers, pills, active states) — noting
the recorded environment limits (backgrounded-tab repaints, synthetic keys).

## Sizing

At or slightly past the one-plan budget (§1 plan-economics). The plan-writer
sizes it; if it overflows, split backend (migration, db functions, filter
query, routes) / frontend (rail, popovers, active-filter row, palette) into
A → B run in sequence — slice 1's precedent. Run `cargo sqlx prepare` once per
plan, before final verification. `export DATABASE_URL=sqlite:portfolio.db`
first — this worktree has no `.env`.
