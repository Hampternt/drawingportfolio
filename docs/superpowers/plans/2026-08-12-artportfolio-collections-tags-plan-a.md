# Artportfolio Collections & Tags — Plan A (backend)

> **For agentic workers:** REQUIRED SUB-SKILLS: `plan-economics` (this repo's task
> classes, review policy and plan sizing) then
> superpowers:subagent-driven-development to execute task-by-task.

**Goal:** Posts gain collections (many-to-many) and tags (normalized, AND-combined),
every post-reading route filters by them server-side through one shared `PostFilter`,
and an admin can assign all of it over new session-gated routes.

**Architecture:** Migration 014 adds four join/lookup tables. One `PostFilter` struct
(parsed once, in `PageQuery::filter`) drives a single `query_as!` macro in
`get_posts_page` — null-tolerant params, `json_each` for the two list params —
replacing the two-variant `match`. `count_posts` grows the same clauses so head
counts track the active filter. Ten new `db.rs` functions cover the collection/tag
CRUD; five new admin routes (plus two GET fragment routes) expose it.

**Slice:** When this plan is done the filter URL contract is live on all three read
routes — `/artportfolio?q=…&tags=…&collection=…&vis=…` filters, paginates and
head-counts correctly, shareable and reload-safe — and every mutation route works
via curl. Deployable: the feed looks unchanged, but the data model and API are
complete. Plan B (`2026-08-12-artportfolio-collections-tags-plan-b.md`) adds the
rail UI, active-filter row, card popovers and styles on top, changing **no SQL**.

Spec: `docs/superpowers/specs/2026-08-12-artportfolio-collections-tags-design.md`.

## Global Constraints

**The URL contract, verbatim from the spec** — all three read routes (`/artportfolio`,
`/artportfolio/htmx/posts`, `/artportfolio/api/posts`) take the same params:

```
/artportfolio?q=loomis&tags=ink,perspective&collection=studies&vis=public,hidden
```

- `tags` — comma-separated, normalized before querying, combine as **AND**.
- `collection` — one slug. Unknown slug filters to zero rows (no 404).
- `vis` — comma-separated subset of `public,unlisted,hidden`. **Admin-only:
  silently ignored for visitors** — the feed is public, and erroring would leak
  that the param exists. Absent ⇒ admin sees all three. A previewing admin
  (`?visitor=1`) is a visitor here too: the effective viewer decides.

**The frozen seam.** Slice 4 codes against the OOB head-count fragment keeping its
exact id and shape:

```html
<div class="hm-eyebrow art-head__label" id="art-head-label" hx-swap-oob="true">…</div>
```

`head_label()` stays the single producer (Task 4 changes its signature, not this
markup). Nothing in this plan may rename or restructure that fragment.

**The pool does not enable SQLite's `foreign_keys` pragma** (`connect()` at
`src/db.rs:13` sets none; `db.rs:1013` and `db.rs:1732` already document working
around this). The migration's `ON DELETE CASCADE` clauses are therefore
documentation, not behaviour — every delete that would rely on them removes its
join rows explicitly, inside a transaction, following the
`delete_task_image` pattern at `src/db.rs:1014`.

**`normalize_tags` rules, verbatim:** trim, lowercase, drop empties, dedupe
preserving first occurrence, max 40 chars per tag (longer ⇒ dropped), max 20 tags
(excess beyond the first 20 dedupe survivors ⇒ dropped). **Collection slugs:**
lowercase, runs of non-alphanumerics → `-`, trimmed of leading/trailing `-`;
duplicate slug on create → 409 carrying the existing collection's name; a name that
slugifies to `""` → 400 (`InvalidName`).

**There is no `.env` in this worktree.** The sqlx macros need a live database
whenever queries change:

```bash
export DATABASE_URL=sqlite:portfolio.db     # once per shell, before any task
```

**`cargo sqlx prepare` runs in Tasks 2 and 3 — the two tasks whose SQL changes —
not only at the plan's end.** This uses the carve-out `plan-economics` §6 names
(*"unless that plan's commits need to build offline individually"*): they do,
because `scripts/verify.sh` runs its tests as `SQLX_OFFLINE=true cargo test
--workspace`, so a task that changes a macro cannot go green against a stale
`.sqlx` cache. Same reasoning, same shape, as the slice-2 plan. Task 6 confirms
the end state with an offline release build.

**Verification for every task:** `./scripts/verify.sh` — all green, output quoted
in the report. Never a bare `cargo test`; the root `Cargo.toml` is both a package
and the workspace root, so it silently skips `drinkinggame`'s tests.

**Browser checkpoints:** none in this plan — it ships no visible UI change. The
final task smoke-tests the URL contract with curl; plan B owns the two browser
checkpoints.

**Scope rules that bite here:** SQL lives in `db.rs` only; templates receive
pre-computed values; every new admin route is gated by the `AuthSession` extractor,
exactly as `patch_visibility` (`src/routes/admin.rs:311`) is.

---

### Task 1: Migration 014, the models, and the two normalizers

**Class:** B

**Why this class:** The migration is `CREATE TABLE IF NOT EXISTS` over four **new**
tables plus one index — it rewrites no existing row, so the machine-checkable
acceptance (idempotence test + the suite) covers it. `normalize_tags` and `slugify`
are pure helpers whose cases and expected values are all below.

**Files:**
- Create: `migrations/014_collections_tags.sql`
- Modify: `src/db.rs` — `run_migrations()` after the 013 block (~line 104), two pub
  helpers near `like_pattern` (~line 115), tests in `mod tests`
- Modify: `src/models.rs` — three new structs + one error enum, after `PostCounts`

**Interfaces:**
- Produces:
  ```rust
  // db.rs
  pub fn normalize_tags(raw: &str) -> Vec<String>;   // comma-separated in
  pub fn slugify(name: &str) -> String;
  // models.rs
  #[derive(Debug, Clone, sqlx::FromRow)]
  pub struct Collection { pub id: i64, pub name: String, pub slug: String, pub created_at: String }
  #[derive(Debug, Clone)]
  pub struct CollectionWithCount { pub id: i64, pub name: String, pub slug: String, pub count: i64 }
  #[derive(Debug, Clone)]
  pub struct TagWithCount { pub name: String, pub count: i64 }
  #[derive(Debug, Clone, PartialEq, Eq)]
  pub enum CreateCollectionError { InvalidName, DuplicateSlug(String) } // payload: existing name
  ```

- [ ] **Step 1: Write the migration**

`migrations/014_collections_tags.sql`, verbatim from the spec:

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

Two notes to carry as SQL comments at the top of the file:

- The `ON DELETE CASCADE` clauses do not fire — the pool never sets
  `PRAGMA foreign_keys` (see Global Constraints). Deletes clean their own join
  rows in Rust, in transactions.
- `idx_posts_visibility_created` arrives here even though the feed's
  OR-with-a-parameter predicate cannot use it — the spec ships it for the
  subquery-driven shapes this migration introduces. It changes no behaviour.

- [ ] **Step 2: Register it in `run_migrations()` and apply it**

Append after the migration 013 block, same `let _ =` shape (every statement is
`IF NOT EXISTS`, so re-runs are no-ops; multi-statement files have precedent —
003 and 011 run the same way):

```rust
    // Migration 014: collections + tags (slice 3). Four new tables and two
    // indexes; no existing row is touched. The REFERENCES clauses are
    // documentation — the pool does not enable foreign_keys, so deletes clean
    // their own join rows in Rust.
    let _ = sqlx::query(include_str!("../migrations/014_collections_tags.sql"))
        .execute(pool)
        .await;
```

Then apply it to the dev database **now**, before any later task's macros need the
tables (`include_str!` touches no database, so this build succeeds first):

```bash
export DATABASE_URL=sqlite:portfolio.db
cargo run    # run_migrations applies 014 at startup; Ctrl-C once it binds :3000
```

- [ ] **Step 3: The two helpers in `db.rs`**

Near `like_pattern` (~line 115), both `pub`:

```rust
pub fn normalize_tags(raw: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for part in raw.split(',') {
        let tag = part.trim().to_lowercase();
        if tag.is_empty() || tag.chars().count() > 40 || out.contains(&tag) {
            continue; // empties, over-length and duplicates are silently dropped
        }
        out.push(tag);
        if out.len() == 20 {
            break; // max 20 tags per post — excess silently dropped
        }
    }
    out
}

pub fn slugify(name: &str) -> String {
    let mut slug = String::new();
    let mut pending_dash = false;
    for c in name.chars() {
        if c.is_alphanumeric() {
            if pending_dash && !slug.is_empty() {
                slug.push('-');
            }
            pending_dash = false;
            for lc in c.to_lowercase() {
                slug.push(lc);
            }
        } else {
            pending_dash = true; // runs of non-alphanumerics collapse to one '-'
        }
    }
    slug // leading/trailing '-' never appear by construction
}
```

- [ ] **Step 4: The models**

Add the four items from the Interfaces block to `src/models.rs` after `PostCounts`.
`CollectionWithCount.count` and `TagWithCount.count` are **viewer-aware** values
computed by Task 2's queries — document that on the structs.

- [ ] **Step 5: Write the tests**

In `src/db.rs`'s `mod tests` (plain `#[test]`, no pool needed except the last):

| Test | Input | Expected |
|---|---|---|
| `test_normalize_tags_trims_and_lowercases` | `" Ink , PERSPECTIVE "` | `["ink", "perspective"]` |
| `test_normalize_tags_drops_empties_and_dupes` | `"ink,,Ink, ink ,wash"` | `["ink", "wash"]` — dedupe preserves first occurrence |
| `test_normalize_tags_drops_over_40_chars` | one 41-char tag + `"ok"` | `["ok"]` |
| `test_normalize_tags_caps_at_20` | 25 distinct tags `t1…t25` | first 20 only |
| `test_normalize_tags_empty_input` | `""` | `[]` |
| `test_slugify_basic` | `"Figure Studies"` | `"figure-studies"` |
| `test_slugify_collapses_runs_and_trims` | `"  Ink & Wash!  "` | `"ink-wash"` |
| `test_slugify_leading_trailing` | `"--Inks--"` | `"inks"` |
| `test_slugify_all_junk_is_empty` | `"!!!"` | `""` |
| `test_migration_014_is_idempotent` | `run_migrations` twice on a fresh pool, then `INSERT INTO tags (name) VALUES ('x')` succeeds | no panic, insert ok |

- [ ] **Step 6: Commit**

```bash
git add migrations/014_collections_tags.sql src/db.rs src/models.rs
git commit -m "feat(artportfolio): the collections and tags tables, and their two normalizers"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

### Task 2: The collections & tags data layer

**Class:** B

**Why this class:** Ten data functions whose test cases and expected values are all
named below. Nothing here reads a session — the gating that makes these dangerous
is wired (and reviewed) in Task 5.

**Files:**
- Modify: `src/db.rs` — ten new functions after `set_post_visibility` (~line 265),
  plus `delete_post_and_get_urls` (~line 315) gains join-row cleanup; tests
- Modify: `.sqlx/` — regenerated

**Interfaces:**
- Consumes: `normalize_tags`, `slugify`, models and `CreateCollectionError` from
  Task 1; `Viewer` (`src/models.rs:127`).
- Produces (exact signatures — the spec's, with the pool param spelled out):
  ```rust
  pub async fn list_collections_with_counts(pool: &DbPool, viewer: Viewer) -> Vec<CollectionWithCount>;
  pub async fn list_tags_with_counts(pool: &DbPool, viewer: Viewer) -> Vec<TagWithCount>;
  pub async fn create_collection(pool: &DbPool, name: &str) -> Result<Collection, CreateCollectionError>;
  pub async fn delete_collection(pool: &DbPool, id: i64) -> bool;
  pub async fn set_post_tags(pool: &DbPool, post_id: i64, tags: &[String]) -> bool; // replace-all
  pub async fn update_post_caption(pool: &DbPool, post_id: i64, caption: &str) -> bool;
  pub async fn add_post_to_collection(pool: &DbPool, post_id: i64, collection_id: i64) -> bool; // idempotent
  pub async fn remove_post_from_collection(pool: &DbPool, post_id: i64, collection_id: i64) -> bool;
  pub async fn get_post_tags(pool: &DbPool, post_id: i64) -> Vec<String>;      // ORDER BY name
  pub async fn get_post_collection_ids(pool: &DbPool, post_id: i64) -> Vec<i64>;
  ```

- [ ] **Step 1: The two list functions**

Counts are **viewer-aware**: a visitor's count is public posts only; an admin's is
every post. Join semantics, pinned:

- `list_collections_with_counts` — `LEFT JOIN`, so an **empty collection appears
  for an admin with count 0** (they just created it from the rail, and deleting it
  needs a row to click). Visitors get only rows whose visible count is > 0.
  Order: `name` ascending. Two macro branches on `viewer.is_admin()` are fine —
  the admin branch has no visibility condition, the visitor branch counts
  `visibility = 'public'` and keeps `HAVING COUNT(...) > 0`.
- `list_tags_with_counts` — **INNER JOIN** through `post_tags` to `posts`, so an
  orphan tag (no posts) appears for nobody — that is the "naturally hides" the
  spec records. Visitors additionally lose tags whose public count is 0.
  Order: `name` ascending.

Remember `COUNT(*)` needs the `AS "count: i64"` override — sqlx infers it as
`i32` (see the comment on `count_posts`, `src/db.rs:180`).

- [ ] **Step 2: `create_collection` and `delete_collection`**

`create_collection`: trim the name, `slugify` it; empty slug →
`Err(InvalidName)`. Insert; on the UNIQUE violation, re-read the existing row by
slug and return `Err(DuplicateSlug(existing.name))`. Detect the violation by
matching the insert error rather than pre-checking — a pre-check races. The
stored `name` is the trimmed input as typed (display case preserved); only the
slug is normalized.

`delete_collection`: a transaction, following `delete_task_image`
(`src/db.rs:1014`): `DELETE FROM post_collections WHERE collection_id = ?` then
`DELETE FROM collections WHERE id = ?`; return `rows_affected() == 1` from the
second. Posts survive — only membership rows go.

- [ ] **Step 3: The per-post functions**

- `set_post_tags` — replace-all, in a transaction: verify the post exists
  (`SELECT id FROM posts WHERE id = ?` → `false` if not), `DELETE FROM post_tags
  WHERE post_id = ?`, then per tag `INSERT OR IGNORE INTO tags (name) VALUES (?)`,
  `SELECT id FROM tags WHERE name = ?`, `INSERT INTO post_tags (post_id, tag_id)
  VALUES (?, ?)`. Callers pass already-normalized tags; this function does not
  re-normalize (one normalizer, applied at the edges — Task 4's `PageQuery::filter`
  and Task 5's PATCH handler).
- `update_post_caption` — plain `UPDATE posts SET caption = ? WHERE id = ?`,
  `rows_affected() == 1`.
- `add_post_to_collection` — **the FK pragma is off, so a dangling insert would
  succeed silently.** Verify both the post and the collection exist first; return
  `false` if either is missing. Then `INSERT OR IGNORE INTO post_collections
  (post_id, collection_id) VALUES (?, ?)` and return `true` (idempotent: a second
  call is still `true`, still one row).
- `remove_post_from_collection` — `DELETE FROM post_collections WHERE post_id = ?
  AND collection_id = ?`; return `true` (idempotent like its counterpart; the
  route re-renders the checklist either way).
- `get_post_tags` — `SELECT t.name FROM post_tags pt JOIN tags t ON t.id =
  pt.tag_id WHERE pt.post_id = ? ORDER BY t.name`.
- `get_post_collection_ids` — `SELECT collection_id FROM post_collections WHERE
  post_id = ? ORDER BY collection_id`.

- [ ] **Step 4: `delete_post_and_get_urls` cleans its join rows**

Inside its existing transaction (`src/db.rs:315`), after the post row is confirmed
and before the `DELETE FROM posts`: `DELETE FROM post_tags WHERE post_id = ?` and
`DELETE FROM post_collections WHERE post_id = ?`. Without this, deleting a post
leaves dangling join rows that corrupt every count (the cascade the schema
declares never fires — Global Constraints). Orphan **tags** are left in place, per
the spec's recorded trade-off; orphan **join rows** are not.

- [ ] **Step 5: Write the tests**

In `src/db.rs`'s `mod tests`, using `test_pool()` and the existing `seed_caption`
helper (posts default to `public`; use `set_post_visibility` for other states):

| Test | Setup | Expected |
|---|---|---|
| `test_create_collection_slugs_the_name` | create `"Figure Studies"` | `Ok`, `slug == "figure-studies"`, `name == "Figure Studies"` |
| `test_create_collection_duplicate_slug` | create `"Figure Studies"`, then `"figure  studies!"` | `Err(DuplicateSlug(name))` with `name == "Figure Studies"` |
| `test_create_collection_junk_name` | create `"!!!"` | `Err(InvalidName)` |
| `test_delete_collection_unlinks_but_keeps_posts` | 1 post in 1 collection; delete the collection | `true`; post still readable via `get_post_by_id` (admin); `get_post_collection_ids` empty |
| `test_delete_collection_unknown_id` | empty db | `false` |
| `test_set_post_tags_replaces` | set `["ink","wash"]` then `["wash","pencil"]` | `get_post_tags == ["pencil","wash"]` (name order) |
| `test_set_post_tags_empty_clears` | set `["ink"]` then `[]` | `get_post_tags` empty |
| `test_set_post_tags_unknown_post` | no post | `false`, and `tags` table gains no row |
| `test_add_post_to_collection_idempotent` | add twice | both `true`; `get_post_collection_ids.len() == 1` |
| `test_add_post_to_unknown_collection` | post exists, cid 999 | `false`; no join row |
| `test_remove_post_from_collection` | add then remove | membership gone |
| `test_update_post_caption_round_trip` | seed, update, re-read | new caption stored; unknown id → `false` |
| `test_list_collections_counts_are_viewer_aware` | collection with 1 public + 1 hidden member | visitor count `1`, admin count `2` |
| `test_list_collections_empty_hidden_from_visitors` | 1 empty collection | absent for `Visitor`; present with count `0` for `Admin` |
| `test_list_tags_counts_are_viewer_aware` | tag on 1 public + 1 hidden post; tag on hidden only | first: visitor `1` / admin `2`; second: absent for visitor, `1` for admin |
| `test_delete_post_cleans_join_rows` | post with 1 tag + 1 membership; `delete_post_and_get_urls` | `get_post_tags` empty and `get_post_collection_ids` empty for that id |

- [ ] **Step 6: Regenerate the sqlx offline cache**

```bash
cargo sqlx prepare
```

This task added ~15 queries the cache has never seen, and its own acceptance line
runs the tests offline (Global Constraints).

- [ ] **Step 7: Commit**

```bash
git add src/db.rs .sqlx
git commit -m "feat(artportfolio): the data layer learns collections and tags"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

### Task 3: `PostFilter` and the one-macro filter query

**Class:** B

**Why this class:** The query and `count_posts` are data-layer logic whose cases
and expected values are all below, including the visitor/admin visibility rows.
The place where `vis` is (or fails to be) stripped for visitors is Task 4, which
is C.

**Files:**
- Modify: `src/models.rs` — `PostFilter`, after `PostCounts`
- Modify: `src/db.rs` — `get_posts_page` (~line 144), `count_posts` (~line 186), tests
- Modify: `src/routes/feed.rs` — three call sites, interim wiring
- Modify: `src/routes/admin.rs:56` — `htmx_admin_posts`'s call site
- Modify: `.sqlx/` — regenerated

**Interfaces:**
- Consumes: `set_post_tags`, `create_collection`, `add_post_to_collection` (test
  seeding), `like_pattern`, `Viewer`.
- Produces:
  ```rust
  // models.rs — verbatim from the spec
  #[derive(Debug, Clone, Default)]
  pub struct PostFilter {
      pub q: Option<String>,
      pub tags: Vec<String>,          // normalized; empty = no tag filter
      pub collection: Option<String>, // slug
      pub vis: Option<Vec<String>>,   // None = viewer default; Some = admin subset
  }
  // db.rs
  pub async fn get_posts_page(pool: &DbPool, filter: &PostFilter, page: i64, viewer: Viewer) -> Vec<Post>;
  pub async fn count_posts(pool: &DbPool, filter: &PostFilter, viewer: Viewer) -> PostCounts;
  ```

- [ ] **Step 1: `PostFilter` in `models.rs`**

Exactly the struct above. Document the invariants on the fields: `tags` arrive
already normalized, `vis` is `Some` only for an admin (Task 4's parser owns that),
and `q` is the raw search — `like_pattern` is applied inside `db.rs`, not by the
caller.

- [ ] **Step 2: Rewrite `get_posts_page`**

Replace the two-variant `match` with one `query_as!` macro. The SQL, verbatim from
the spec (decision 2026-08-12) — use a raw string so `ESCAPE '\'` needs no
double-escaping:

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

Bind preparation, in placeholder order (six arguments, one per placeholder
number — a numbered placeholder reused in the SQL still takes exactly one
argument):

```rust
let all = viewer.is_admin();                                  // ?1
let pattern = filter.q.as_deref().map(like_pattern);          // ?2  Option<String>
let collection = filter.collection.as_deref();                // ?3  Option<&str>
let tags_json = if filter.tags.is_empty() {                   // ?4  Option<String>
    None
} else {
    Some(serde_json::to_string(&filter.tags).unwrap())
};
let vis_json = filter                                          // ?5  Option<String>
    .vis
    .as_ref()
    .map(|v| serde_json::to_string(v).unwrap());
let offset = page * 20;                                        // ?6
```

`?4`/`?5` are JSON array strings (e.g. `["ink","perspective"]`) or SQL NULL —
`json_each(NULL)` yields no rows and the `IS NULL` arm has already won. N+1
pagination is unchanged: 21 rows requested, caller drops the 21st.

Rewrite the function's doc comment: the two-branch explanation and the "no index
is created" paragraph (~lines 129–143) are both obsolete — one macro now, and
migration 014 shipped `idx_posts_visibility_created`. Say what replaced them and
why (`json_each` keeps the macro compile-time checked with list params).

- [ ] **Step 3: `count_posts` grows the same clauses**

Same shape as today — `GROUP BY visibility`, no viewer clause in the SQL, viewer
decides `total` in Rust (`src/db.rs:183`'s comment still holds). One macro now:

```sql
SELECT visibility AS "visibility!: String", COUNT(*) AS "n: i64"
FROM posts WHERE
    (?1 IS NULL OR caption LIKE ?1 ESCAPE '\')
AND (?2 IS NULL OR id IN
     (SELECT post_id FROM post_collections pc
      JOIN collections c ON c.id = pc.collection_id WHERE c.slug = ?2))
AND (?3 IS NULL OR id IN
     (SELECT post_id FROM post_tags pt
      JOIN tags t ON t.id = pt.tag_id
      WHERE t.name IN (SELECT value FROM json_each(?3))
      GROUP BY post_id
      HAVING COUNT(DISTINCT t.id) = json_array_length(?3)))
AND (?4 IS NULL OR visibility IN (SELECT value FROM json_each(?4)))
GROUP BY visibility
```

The accumulation loop and the `total` computation are unchanged. The `AS "n: i64"`
override stays — sqlx infers `COUNT(*)` as `i32`.

- [ ] **Step 4: Update the callers — interim wiring**

Task 4 owns the real parse; this step only keeps everything compiling with
today's behaviour:

| Call site | Interim value |
|---|---|
| `feed.rs` `render_grid` / `feed_page` / `htmx_posts` / `api_posts` | build `PostFilter { q, ..Default::default() }` from the already-normalized `q` where each currently passes `q.as_deref()`; thread `&PostFilter` through `render_grid`'s `q` parameter (rename it `filter`) |
| `admin.rs:56` `htmx_admin_posts` | `&PostFilter::default()` — the dashboard lists everything, and `Viewer::Admin` stays exactly as the comment there demands |
| `db.rs` tests | explicit `PostFilter` per test |

Behaviour after this step is byte-identical to before it; the plan's commits stay
individually deployable.

- [ ] **Step 5: Write the tests**

In `src/db.rs`'s `mod tests`. Seed with `seed_caption` + `set_post_visibility` +
Task 2's functions. A helper keeps them readable:

```rust
fn tag_filter(tags: &[&str]) -> PostFilter {
    PostFilter { tags: tags.iter().map(|s| s.to_string()).collect(), ..Default::default() }
}
```

| Test | Setup | Expected |
|---|---|---|
| `test_filter_multi_tag_is_and` | post A tags `["ink"]`, post B tags `["ink","perspective"]` | `tags=["ink","perspective"]` → only B; `tags=["ink"]` → A and B |
| `test_filter_unknown_collection_is_empty` | 2 public posts | `collection = Some("no-such-slug")` → `[]`, no error |
| `test_filter_collection_scopes` | post A in `studies`, post B not | `collection = Some("studies")` → only A |
| `test_filter_like_escape_with_tags` | post `"100% ink study"` tagged `ink`; post `"loose gesture"` tagged `ink` | `q = Some("100%")` + `tags=["ink"]` → 1 row |
| `test_filter_visitor_stays_public_with_tags` | tagged post public, tagged post hidden, same tag | visitor + that tag → public one only; admin → both |
| `test_filter_vis_subset` | 1 public, 1 unlisted, 1 hidden | admin + `vis = Some(vec!["hidden"])` → hidden only; `vis = Some(vec!["public","unlisted"])` → 2 rows |
| `test_filter_default_matches_slice2_behaviour` | 1 public, 1 hidden | `PostFilter::default()`: visitor → 1, admin → 2 |
| `test_filter_keeps_n_plus_1_probe` | 22 public posts | `PostFilter::default()`, page 0 → `len() == 21` |
| `test_count_posts_reflects_tag_filter` | 2 public tagged `ink`, 1 public untagged, 1 hidden tagged `ink` | `tags=["ink"]`: visitor `total == 2`; admin `total == 3`, `hidden == 1` |
| `test_count_posts_reflects_vis_subset` | same seed | admin + `vis=["hidden"]` → `total == 1`, `public == 0` |

- [ ] **Step 6: Regenerate the sqlx offline cache**

```bash
cargo sqlx prepare
```

Both rewritten macros changed; the acceptance line runs offline.

- [ ] **Step 7: Commit**

```bash
git add src/models.rs src/db.rs src/routes/feed.rs src/routes/admin.rs .sqlx
git commit -m "feat(artportfolio): one query that answers every filter"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

### Task 4: `PageQuery::filter` — the URL contract through the three read routes

**Class:** C

**Why this class:** This is where `vis` is silently stripped for visitors — an
auth-adjacent decision a reviewer must see wired, not just tested — and where the
filter has to survive the pagination boundary that lives in generated URLs. It
also touches `head_label`, whose OOB fragment is the frozen seam.

**Files:**
- Modify: `src/routes/feed.rs` — `PageQuery`, new `filter()` method, `filter_desc`,
  `head_label`, `append_filter_pairs`, `load_more_url`, `page_url`, `render_grid`,
  all four handlers, tests

**Interfaces:**
- Consumes: `PostFilter`, `get_posts_page`, `count_posts` (Task 3);
  `normalize_tags` (Task 1); existing `normalize_q`, `effective_viewer`,
  `is_preview`.
- Produces:
  ```rust
  pub struct PageQuery {
      pub page: Option<i64>,
      pub q: Option<String>,
      pub last_month: Option<String>,
      pub visitor: Option<String>,
      pub tags: Option<String>,        // comma-separated, raw
      pub collection: Option<String>,  // slug, raw
      pub vis: Option<String>,         // comma-separated, raw — admin-only in effect
  }
  impl PageQuery {
      /// The single owner of the parse. `viewer` must be the EFFECTIVE viewer.
      pub fn filter(&self, viewer: Viewer) -> PostFilter;
  }
  fn filter_desc(filter: &PostFilter) -> Option<String>;
  fn head_label(counts: &PostCounts, filter: &PostFilter, viewer: Viewer) -> String;
  fn append_filter_pairs(s: &mut url::form_urlencoded::Serializer<'_, String>,
                         filter: &PostFilter, preview: bool);
  fn load_more_url(next_page: i64, filter: &PostFilter, last_month: Option<&str>, preview: bool) -> String;
  fn page_url(filter: &PostFilter, preview: bool) -> String;
  async fn render_grid(state, page, filter: &PostFilter, last_month, head_label_oob, viewer, preview) -> String;
  ```
  Plan B builds its rail-toggle URLs on `append_filter_pairs` — the pair order
  below is a contract, not a style choice.

- [ ] **Step 1: `PageQuery` grows the three fields, and `filter()` owns the parse**

```rust
impl PageQuery {
    pub fn filter(&self, viewer: Viewer) -> PostFilter {
        let vis = if viewer.is_admin() {
            self.vis.as_deref().map(|raw| {
                let mut list: Vec<String> = Vec::new();
                for v in raw.split(',').map(str::trim) {
                    // unknown states are dropped, duplicates kept once,
                    // first-occurrence order preserved
                    if ["public", "unlisted", "hidden"].contains(&v)
                        && !list.iter().any(|x| x == v)
                    {
                        list.push(v.to_string());
                    }
                }
                list
            })
            .filter(|v| !v.is_empty())
        } else {
            // Silently ignored for visitors — the feed is public, and a 4xx
            // here would leak that the param exists. A previewing admin is a
            // visitor by the time this runs, because the caller passes the
            // EFFECTIVE viewer.
            None
        };
        PostFilter {
            q: normalize_q(self.q.as_deref()),
            tags: self.tags.as_deref().map(crate::db::normalize_tags).unwrap_or_default(),
            collection: self.collection.as_deref().map(str::trim)
                .filter(|s| !s.is_empty()).map(str::to_string),
            vis,
        }
    }
}
```

A `vis` that parses to nothing is `None` (viewer default), never `Some(vec![])` —
`Some(vec![])` would filter to zero rows.

- [ ] **Step 2: `filter_desc` — one description, two consumers**

The head label and (in plan B) the empty state both echo the active filter, so
one function owns the wording. Pinned shape: tags in their given order, then the
quoted search, then the collection slug, all joined by ` + `:

```rust
/// `perspective + ink + "loomis" + studies`, or None when nothing filters.
/// The vis subset is deliberately absent — it is admin plumbing, not a search.
fn filter_desc(filter: &PostFilter) -> Option<String> {
    let mut parts: Vec<String> = filter.tags.clone();
    if let Some(q) = &filter.q {
        parts.push(format!("\"{q}\""));
    }
    if let Some(c) = &filter.collection {
        parts.push(c.clone());
    }
    if parts.is_empty() { None } else { Some(parts.join(" + ")) }
}
```

- [ ] **Step 3: `head_label` takes the filter**

Signature per Interfaces. Body: where it currently branches on `q`
(`src/routes/feed.rs:67`), branch on `filter_desc(filter)` instead — `Some(desc)`
renders `{total} {noun} · matching {desc}{tail}`; `None` keeps today's two
no-search shapes exactly. When the only filter is a search, `desc` is `"cat"`
(quoted), so the label is **byte-identical to slice 2's** `matching "cat"` — the
existing head-label tests must keep passing unmodified.

**The frozen seam:** this function's output still lands in
`<div class="hm-eyebrow art-head__label" id="art-head-label" hx-swap-oob="true">`
via `post_grid.html` — id and shape untouched, `head_label()` the single producer.

- [ ] **Step 4: The URL builders**

`append_filter_pairs` writes, in this order and only when present: `q`, `tags`
(comma-joined — the serializer will percent-encode the commas to `%2C`, which is
fine), `collection`, `vis` (comma-joined), then `visitor=1` when `preview`.
`load_more_url` becomes: `page` pair, then `append_filter_pairs`, then
`last_month` — it and `page_url` both delegate so the two cannot drift, and plan
B's rail URLs join them on the same helper.

`page_url`'s early return (`src/routes/feed.rs:156`) becomes "no q, no tags, no
collection, no vis, no preview → `/artportfolio`".

- [ ] **Step 5: Wire the four handlers**

Each computes `preview` / `viewer` as today, then **one**
`let filter = query.filter(viewer);` and threads `&filter` everywhere the interim
`PostFilter { q, .. }` from Task 3 sat: `render_grid`, `count_posts`,
`head_label`, `load_more_url` (via `render_grid`), `page_url` (the `HX-Push-Url`
header — this is what makes a rail-driven filter survive reload once plan B
lands). `api_posts` passes `effective_viewer(session_is_admin, false)` into
`query.filter(...)` — same params, no preview, JSON shape unchanged (the recorded
trade-off: no tags/collections in the payload).

`FeedTemplate.q` keeps feeding the search input from `filter.q`.

- [ ] **Step 6: Write the tests**

In `feed.rs`'s `mod tests`, reusing `app_with_pool`, `seed`, `admin_cookie`,
`fragment`, `body_of`:

| Test | Expected |
|---|---|
| `test_pagequery_filter_drops_vis_for_visitor` | `vis: Some("hidden".into())` + `Viewer::Visitor` → `filter.vis == None` |
| `test_pagequery_filter_keeps_vis_for_admin` | same + `Viewer::Admin` → `Some(vec!["hidden"])` |
| `test_pagequery_filter_drops_junk_vis` | `"public,bogus"` admin → `Some(vec!["public"])`; `"bogus"` admin → `None` |
| `test_pagequery_filter_normalizes_tags` | `tags: Some("Ink, ink,,PERSPECTIVE".into())` → `["ink","perspective"]` |
| `test_route_vis_is_ignored_for_visitors` | seed public `"pub-cat"` + hidden `"hid-cat"`; GET `/artportfolio/htmx/posts?vis=hidden` with no cookie → body contains `pub-cat`, not `hid-cat` |
| `test_route_vis_subset_for_admin` | same seed; with cookie, `?vis=hidden` → `hid-cat` only |
| `test_route_tags_filter_applies` | tag one post `ink` (via `set_post_tags`); `?tags=ink` returns it alone |
| `test_load_more_url_carries_filters` | `load_more_url(1, &f, None, false)` with q `loomis`, tags `[ink, perspective]`, collection `studies` == `"/artportfolio/htmx/posts?page=1&q=loomis&tags=ink%2Cperspective&collection=studies"` |
| `test_page_url_carries_filters` | `page_url(&f, true)` with tags `[ink]` == `"/artportfolio?tags=ink&visitor=1"` |
| `test_pagination_carries_the_filter` | GET `/artportfolio/htmx/posts?page=0&tags=ink` (21+ tagged posts seeded, or just assert on the button) → body's Load more URL contains `tags=ink` and `page=1` |
| `test_head_label_with_tags_and_search` | visitor, `total 12`, tags `[ink, perspective]`, q `loomis` → `12 drawings · matching ink + perspective + "loomis"` |
| `test_head_label_collection_only` | visitor, `total 3`, collection `studies` → `3 drawings · matching studies` |
| `test_head_label_plain_search_unchanged` | visitor, `total 12`, q `cat` only → `12 drawings · matching "cat"` — slice 2's exact string |
| `test_api_posts_honours_tags` | 2 public posts, one tagged; `/artportfolio/api/posts?tags=ink` → JSON lists 1 |
| `test_oob_label_keeps_the_frozen_seam` | GET `/artportfolio/htmx/posts?page=0&tags=ink` → body contains `id="art-head-label" hx-swap-oob="true"` |

- [ ] **Step 7: Commit**

```bash
git add src/routes/feed.rs
git commit -m "feat(artportfolio): the filter URL contract on every read route"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

### Task 5: The admin mutation routes and their fragments

**Class:** C

**Why this class:** Seven new session-gated routes. `AuthSession` extractors are
the only wall between a visitor and rewriting the portfolio's organization — the
kind of gating a reviewer verifies wired, not inferred from green tests.

**Files:**
- Modify: `src/routes/admin.rs` — seven handlers, three template structs,
  `router()`, tests
- Create: `templates/artportfolio/partials/rail_collections.html`
- Create: `templates/artportfolio/partials/card_edit_popover.html`
- Create: `templates/artportfolio/partials/collection_checklist.html`

**Interfaces:**
- Consumes: every Task 2 function; `normalize_tags`; `PostCardTemplate`
  (`src/routes/feed.rs:176`); `get_post_by_id`.
- Produces — the route table, spec verbatim plus the two GET fragment routes
  (resolved ambiguity: the popovers must *prefill* — an empty tags input plus
  replace-all PATCH semantics would silently wipe a post's tags — and `Post`
  carries no tags, so the popover content is fetched, which needs a GET):

  | Route | Body (form) | Response |
  |---|---|---|
  | `POST /api/admin/collections` | `name` | **201** + rail fragment; **409** duplicate slug (body carries the existing collection's name); **400** `InvalidName` |
  | `DELETE /api/admin/collections/{id}` | — | **200** + rail fragment (idempotent — unknown id still renders the fragment) |
  | `PATCH /api/admin/posts/{id}` | `caption`, `tags` (comma-sep) | **200** + re-rendered card (`closest .hm-post` outerHTML, the slice-2 pattern); **404** unknown id |
  | `POST /api/admin/posts/{id}/collections/{cid}` | — | **200** + refreshed checklist fragment; **404** when post or collection is unknown |
  | `DELETE /api/admin/posts/{id}/collections/{cid}` | — | same |
  | `GET /api/admin/posts/{id}/edit` | — | **200** + edit-popover fragment; **404** unknown id |
  | `GET /api/admin/posts/{id}/collections` | — | **200** + checklist fragment; **404** unknown id |

  Fragment contracts plan B builds against:
  - `rail_collections.html` root: `<div id="rail-collections">` — consumes
    `collections: Vec<CollectionWithCount>` and `is_admin: bool` from scope.
  - `collection_checklist.html` root: `<div class="art-checklist"
    id="art-checklist-{{ post_id }}">` — consumes `post_id: i64`,
    `items: Vec<ChecklistItem>` where
    `struct ChecklistItem { pub id: i64, pub name: String, pub member: bool }`.
  - `card_edit_popover.html` — consumes `post_id: i64`, `caption: String`,
    `tags_joined: String` (comma-space joined).

- [ ] **Step 1: The three fragment templates, minimal but real**

Plan B restyles these in place (and `filter_rail.html` will `include`
`rail_collections.html`, sharing field names with `FeedTemplate`) — so keep the
markup semantic and the ids exact; classes may be plain for now.

- `rail_collections.html` — one row per collection: a link to
  `/artportfolio?collection={{ c.slug }}`, the name, the count in a
  `<span class="art-rail__count">`, and behind `{% if is_admin %}` a delete
  button with `hx-delete="/api/admin/collections/{{ c.id }}"`,
  `hx-target="#rail-collections"`, `hx-swap="outerHTML"`,
  `hx-confirm="Delete this collection? Posts are not deleted."`.
- `card_edit_popover.html` — a form: `<textarea name="caption">` prefilled,
  `<input name="tags">` prefilled with `tags_joined`, a Save button with
  `hx-patch="/api/admin/posts/{{ post_id }}"`,
  `hx-target="closest .hm-post"`, `hx-swap="outerHTML"` — the exact swap contract
  `patch_visibility`'s buttons already use (`templates/partials/post_card.html:38`).
- `collection_checklist.html` — one label+checkbox per item, `checked` when
  `member`; each checkbox fires `hx-post` (unchecked→checked) or `hx-delete`
  (checked→unchecked) to `/api/admin/posts/{{ post_id }}/collections/{{ item.id }}`
  with `hx-target="#art-checklist-{{ post_id }}"`, `hx-swap="outerHTML"`. Two
  static attributes per state branch beat client-side logic here — the fragment
  re-renders wholesale on every toggle, so each render knows which verb each row
  needs. When `items` is empty, one muted line: `No collections yet.`

Corresponding Askama structs live in `admin.rs`:
`RailCollectionsTemplate { collections, is_admin }`,
`CardEditTemplate { post_id, caption, tags_joined }`,
`CollectionChecklistTemplate { post_id, items }`.

- [ ] **Step 2: The handlers**

All seven take `_session: crate::middleware::AuthSession` first — follow
`patch_visibility` (`src/routes/admin.rs:311`) for extractor order, `Form` use
and error shape. Notes beyond the table:

- `create_collection_route` — trim the name; map `Err(InvalidName)` → 400,
  `Err(DuplicateSlug(name))` → 409 with body
  `A collection named "{name}" already exists.`; on `Ok` re-fetch
  `list_collections_with_counts(pool, Viewer::Admin)` and respond
  `(StatusCode::CREATED, Html(fragment))`.
- `delete_collection_route` — call `delete_collection`, ignore the bool
  (idempotent by contract), render the fragment.
- `patch_post` — `Form { caption: String, tags: String }`. Normalize:
  `let tags = crate::db::normalize_tags(&form.tags);`. Call
  `update_post_caption` (false → 404) then `set_post_tags`. Success body is the
  re-read card, exactly as `patch_visibility` builds it (`get_post_by_id` with
  `Viewer::Admin`, `PostCardTemplate { post, is_first: false, is_admin: true }`).
- membership handlers — `Path((id, cid)): Path<(i64, i64)>`;
  `add_post_to_collection` / `remove_post_from_collection` returning `false` on
  a missing post/collection → 404; then render the checklist from
  `list_collections_with_counts(pool, Viewer::Admin)` ∩ `get_post_collection_ids`.
  Factor that assembly into one `async fn checklist_fragment(pool, post_id)` used
  by all three checklist-returning routes.
- `edit_post_fragment` — `get_post_by_id(pool, id, Viewer::Admin)` (404 on None),
  `get_post_tags`, join with `", "`.

- [ ] **Step 3: Register the routes**

In `router()` (`src/routes/admin.rs:424`). **Axum panics at startup on a second
`.route()` call for a path it already has** — `/api/admin/posts/{id}` is already
registered for DELETE, so PATCH must merge into that same call:

```rust
.route("/api/admin/posts/{id}", delete(delete_post).patch(patch_post))
.route("/api/admin/collections", post(create_collection_route))
.route("/api/admin/collections/{id}", delete(delete_collection_route))
.route(
    "/api/admin/posts/{id}/collections",
    get(collections_checklist_fragment),
)
.route(
    "/api/admin/posts/{id}/collections/{cid}",
    post(add_post_collection).delete(remove_post_collection),
)
.route("/api/admin/posts/{id}/edit", get(edit_post_fragment))
```

(`patch` is already imported for `patch_visibility`.)

- [ ] **Step 4: Write the tests**

In `admin.rs`'s `mod tests`, reusing `app_with_pool`, `seed_post`,
`admin_cookie` — and following `test_patch_visibility_requires_session`'s shape
for the auth cases:

| Test | Expected |
|---|---|
| `test_collections_routes_require_session` | no cookie: POST `/api/admin/collections`, DELETE `…/collections/1`, PATCH `…/posts/1`, POST + DELETE `…/posts/1/collections/1`, GET `…/posts/1/edit`, GET `…/posts/1/collections` — **every one** non-200 (redirect or 401), asserted in one loop |
| `test_create_collection_201_with_fragment` | POST `name=Figure Studies` → 201, body contains `id="rail-collections"` and `Figure Studies` |
| `test_create_collection_duplicate_is_409` | create twice → 409, body contains `Figure Studies` (the existing name) |
| `test_create_collection_junk_name_is_400` | `name=!!!` → 400 |
| `test_delete_collection_returns_fragment` | 200, body contains `id="rail-collections"`, collection gone from db |
| `test_patch_post_updates_caption_and_tags` | PATCH `caption=New caption&tags=Ink, wash` → 200, body contains `hm-post` and `New caption`; `get_post_tags` == `["ink","wash"]` |
| `test_patch_post_replaces_tags` | pre-set `["old"]`, PATCH `tags=new` → `get_post_tags == ["new"]` |
| `test_patch_post_unknown_id_is_404` | 404 |
| `test_membership_add_then_remove` | POST → 200, body contains `art-checklist` and a `checked`; DELETE → 200, no `checked`; db agrees via `get_post_collection_ids` |
| `test_membership_unknown_collection_is_404` | POST with cid 999 → 404 |
| `test_edit_fragment_prefills` | seed caption `"Old"` + tags `["ink"]`; GET edit → body contains `Old` and `ink` |

- [ ] **Step 5: Commit**

```bash
git add src/routes/admin.rs templates/artportfolio/partials/rail_collections.html templates/artportfolio/partials/card_edit_popover.html templates/artportfolio/partials/collection_checklist.html
git commit -m "feat(artportfolio): routes to organise — collections, tags, captions"
```

**Acceptance:** `./scripts/verify.sh` — all green.

No `cargo sqlx prepare` here: every query this task calls was prepared in Tasks
2–3, and Askama templates compile into the binary.

---

### Task 6: Offline build, curl smoke, docs

**Class:** A

**Why this class:** Mechanical. The offline build either succeeds or it does not;
the docs are prose.

**Files:**
- Modify: `CLAUDE.md` — migration list, route list, models paragraph, test counts
- Modify: `docs/WORKTREES.md` — the artportfolio card

- [ ] **Step 1: Confirm the offline end state**

```bash
SQLX_OFFLINE=true cargo build --release
```

A failure means a query changed after its `prepare` — run `cargo sqlx prepare`
once more and commit the diff.

- [ ] **Step 2: Smoke the URL contract**

`cargo run`, then eyeball (this substitutes for a browser checkpoint — the plan
ships no UI):

```bash
curl -s 'http://localhost:3000/artportfolio/api/posts?tags=ink' | head -c 400
curl -s 'http://localhost:3000/artportfolio/htmx/posts?page=0&collection=studies&vis=hidden' | head -c 400
```

The second, cookie-less, must show public rows only — `vis` ignored for visitors.

- [ ] **Step 3: Update the docs**

`CLAUDE.md`: migrations gain "014 collections/tags"; the admin route list gains
the seven routes; the models paragraph gains `PostFilter`, `Collection`,
`CollectionWithCount`, `TagWithCount`. **Re-measure the test counts** —
`cargo test --workspace 2>&1 | grep "test result"` — never copy a number from
another document.

`docs/WORKTREES.md`: the artportfolio card's Status becomes "slice 3 plan A
(backend) complete — filter contract + admin routes live, no UI yet"; Next is
plan B. Record the resolved ambiguities so plan B and slice 4 do not re-litigate
them: the FK pragma is off so joins are cleaned manually; the two extra GET
fragment routes exist and why; rail-count staleness after card edits is accepted
per the spec.

- [ ] **Step 4: Commit**

```bash
git add CLAUDE.md docs/WORKTREES.md
git commit -m "docs: slice 3 plan A lands — the filter backend"
```

**Acceptance:** `./scripts/verify.sh` — all green, plus
`SQLX_OFFLINE=true cargo build --release` succeeding.

---

## Before the plan is done

- Every task classed; both C tasks (4, 5) got their per-task reviewer.
- Then **one review of the whole plan's diff** on the most capable model — the
  only review Tasks 1–3 and 6 get. Point it at the Global Constraints and ask
  specifically:
  1. Can any route be made to return a post its viewer may not see, through any
     combination of `tags` / `collection` / `vis` / `visitor` params? (`vis` for
     a visitor and for a previewing admin are the sharp edges.)
  2. Is every new admin route actually behind `AuthSession` — including the two
     GET fragment routes, which leak hidden posts' captions and tags if not?
  3. Do any deletes still rely on the cascade that never fires?
  4. Did the OOB head fragment keep its exact id and shape?
