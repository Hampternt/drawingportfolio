# Artportfolio Visibility Model — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILLS: `plan-economics` (this repo's task
> classes, review policy and plan sizing) then
> superpowers:subagent-driven-development to execute task-by-task.

**Goal:** Give every post a `public` / `unlisted` / `hidden` state, enforce it on
every route that reads posts, and let an admin change it inline from the feed.

**Architecture:** A `visibility` column (migration 013) plus two enums in
`models.rs` — `Visibility` for the state and `Viewer` for who is asking. Every
`db.rs` function that reads posts takes a `Viewer`; each handler derives **one**
effective viewer from `OptionalAuth` and the `?visitor=1` preview flag, and drives
both its db call and its templates from that single value.

**Slice:** All of slice 2. When this plan is done the visibility model is complete
and deployable — the three states are real, enforced on all four post-reading
routes, changeable from the feed without a reload, settable at upload time, visible
as a per-card badge, summarised in the page head, and previewable through
`?visitor=1`. Slice 3 (collections, tags, the full filter rail) picks up next and
depends on nothing here beyond the `Viewer` parameter already being threaded.

Spec: `docs/superpowers/specs/2026-08-11-artportfolio-visibility-model-design.md`.

**On the size of this plan.** Eight tasks exceeds `plan-economics` §1, which sizes a
plan at 4–6 tasks and one session, at the user's explicit direction — they asked for
one plan covering everything slice 2 needs rather than the A/B split the spec
proposed. The risk is the measured one: the 2026-08-01 fitness redesign's 421-turn,
139M-cache-read session came from exactly this, a plan whose scope outran a single
session. Two mitigations are built in — Tasks 1–4 end at a deployable state (the
model enforced, nothing yet able to change it), so **that is a clean place to end a
session and resume from the ledger** if context is running long by then.

## Global Constraints

**The three states, verbatim from the spec.** This table is the acceptance criterion
for Tasks 2, 3, 4 and 5:

| visibility | in feed | in JSON API | permalink, visitor | permalink, admin |
|---|---|---|---|---|
| `public`   | yes | yes | 200 | 200 |
| `unlisted` | no  | no  | 200 | 200 |
| `hidden`   | no  | no  | **404** | 200 |

`hidden` returns **404, never 403**. A 403 confirms the row exists, which is the one
fact hiding it is meant to withhold.

**Fail closed, in two different ways.** Do not collapse these:

- Parsing a value **read from the DB**: unrecognised → `Hidden`. A corrupt or
  future value must not render to the public.
- Parsing **input from outside** — the `PATCH` body, the upload's multipart field:
  unrecognised → `400`. Silently coercing a typo to `Hidden` would look like the
  request succeeded.

**One effective viewer.** `Viewer` governs the db layer and a template flag governs
the markup, and they must be derived from the same value:

```rust
let effective = if session_is_admin && !preview { Viewer::Admin } else { Viewer::Visitor };
```

The raw `OptionalAuth` bool survives for exactly one purpose — rendering the
"previewing as a visitor / exit" affordance, which a real visitor must never see.
Everything else reads `effective`.

**The preview flag rides with `q`.** `?visitor=1` must be threaded through
`PageQuery`, `load_more_url()`, `page_url()` and `head_label()` exactly as `q` and
`last_month` already are. Miss the URL builders and page 0 renders as a visitor while
the first *Load more* renders as an admin.

**Migration numbering follows ship order.** Slice 1 shipped image dimensions as 012,
so visibility is **013** — not the 012 the handoff reserved for it.

**No `posts(visibility, created_at DESC)` index**, though the handoff lists one. See
Task 2: the query shape cannot use it.

**Verification for every task:** `./scripts/verify.sh` — all green, output quoted in
the report. Never a bare `cargo test`; the root `Cargo.toml` is both a package and
the workspace root, so it silently skips `drinkinggame`'s tests.

**`cargo sqlx prepare` runs in every task that touches SQL — Tasks 1, 2 and 5 — not
once at the end.** This deliberately departs from `plan-economics` §6, using the
carve-out it names: *"unless that plan's commits need to build offline
individually."* They do. `scripts/verify.sh` runs its test step as
`SQLX_OFFLINE=true cargo test --workspace`, so the moment Task 1 adds `visibility`
to `Post` and to every posts `SELECT`, a stale `.sqlx` cache fails that task's own
acceptance line.

**There is no `.env` in this worktree**, so nothing exports `DATABASE_URL` for you.
The sqlx macros need a live database to infer against whenever the queries change:

```bash
export DATABASE_URL=sqlite:portfolio.db     # once per shell, before any task
```

The ordering trap this creates is spelled out in Task 1, Step 2 — read it before
touching `models.rs`.

**Browser checkpoints:** after Task 6 (the visual layer) and before the final review.
Not per task.

---

### Task 1: Migration 013 and the `Visibility` enum

**Class:** C

**Why this class:** A migration over existing rows. Every post in the live database
gets a value it never had, and `Post`'s row shape changes under six `.sqlx` cache
entries. A machine can tell you it compiled; it cannot tell you the backfill meant
what you intended.

**Files:**
- Create: `migrations/013_post_visibility.sql`
- Modify: `src/db.rs` — `run_migrations()`, after the migration 012 block
- Modify: `src/models.rs` — `Post` struct (~line 4), new `Visibility` enum after `PostFormat` (~line 61)
- Modify: every `SELECT` in `src/db.rs` that names the posts columns explicitly

**Interfaces:**
- Produces:
  ```rust
  pub enum Visibility { Public, Unlisted, Hidden }
  impl Visibility {
      pub fn as_str(&self) -> &'static str;
      pub fn from_str(s: &str) -> Option<Self>;      // None on unrecognised
      pub fn from_row(s: &str) -> Self;              // fail closed: unrecognised -> Hidden
  }
  impl Default for Visibility { fn default() -> Self { Self::Public } }
  // Post gains:
  pub visibility: String,
  ```

- [ ] **Step 1: Write the migration**

`migrations/013_post_visibility.sql`:

```sql
-- Visibility model: public / unlisted / hidden.
--
-- Existing rows become 'public', which is what they already were in effect --
-- every post has been publicly listed since the feed existed, so this backfill
-- changes no behaviour, it only names the behaviour that was already there.
--
-- No CHECK constraint. SQLite cannot add one to an existing table without a
-- full table rebuild, and the value is validated in Rust on the way in
-- (Visibility::from_str) and again on the way out (Visibility::from_row, which
-- fails closed to Hidden). A constraint here would buy a rebuild and duplicate
-- a guarantee we already have.
ALTER TABLE posts ADD COLUMN visibility TEXT NOT NULL DEFAULT 'public';
```

Deliberately **no index**. See Task 2's Step 2 for why the query shape cannot use
one.

- [ ] **Step 2: Register it in `run_migrations()`, then apply it — before touching `models.rs`**

**Order matters here and the obvious order deadlocks.** The sqlx macros infer
against a live database, so once `Post` names `visibility` nothing compiles until the
column exists — and applying the migration by running the app needs a build. Do the
registration and the run *first*, while no query mentions the new column.

Append after the migration 012 block, following the `let _ =` duplicate-column
tolerance every migration from 002 onward uses:

```rust
    // Migration 013: visibility model — public / unlisted / hidden.
    //
    // Existing rows default to 'public'. Slice 1 took 012 for image dimensions,
    // so this is 013 even though the design handoff reserved 012 for it —
    // migration numbers follow ship order, not the order a document listed them.
    let _ = sqlx::query(include_str!("../migrations/013_post_visibility.sql"))
        .execute(pool)
        .await;
```

Then apply it:

```bash
cargo run    # run_migrations applies 013 at startup; Ctrl-C once it binds :3000
```

`run_migrations` pulls the file in with `include_str!`, which is not a macro that
touches the database, so this build succeeds while the column is still absent. Steps
3 and 4 then edit `models.rs` and the queries against a database that already has it.

- [ ] **Step 3: Add the enum to `models.rs`**

Follow the `PostFormat` shape directly above it (`as_str`, `Default`), adding the two
parse functions:

```rust
/// A post's visibility state.
///
/// Stored as TEXT. `Post.visibility` holds the raw string rather than this enum
/// because six queries select the posts columns and each would otherwise need a
/// sqlx type override; parsing happens where behaviour depends on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    Public,
    Unlisted,
    Hidden,
}

impl Visibility {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Unlisted => "unlisted",
            Self::Hidden => "hidden",
        }
    }

    /// Strict parse, for input arriving from outside — a PATCH body or a
    /// multipart field. `None` is a 400 at the call site, never a default.
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "public" => Some(Self::Public),
            "unlisted" => Some(Self::Unlisted),
            "hidden" => Some(Self::Hidden),
            _ => None,
        }
    }

    /// Lenient parse, for a value read back out of the database — and it fails
    /// **closed**. A corrupt row, or one written by a future version that knows
    /// a state this build does not, must not render to the public.
    pub fn from_row(s: &str) -> Self {
        Self::from_str(s).unwrap_or(Self::Hidden)
    }
}

impl Default for Visibility {
    fn default() -> Self {
        Self::Public
    }
}
```

- [ ] **Step 4: Add the field to `Post` and to every explicit column list**

`Post` gains `pub visibility: String`, documented as raw column text.

Then update every `SELECT` in `db.rs` that spells the posts columns out. They all
carry the same list today:

```
id, caption, image_url, webp_url, avif_url, format, file_size_bytes, created_at, image_width, image_height
```

becomes that list plus `, visibility`. Find them with:

```bash
grep -n "image_width, image_height FROM posts" src/db.rs
```

`insert_post` does **not** gain the column here — new posts take the column default
until Task 5 adds the upload's field.

- [ ] **Step 5: Regenerate the sqlx offline cache**

```bash
cargo sqlx prepare
```

Not deferred to the end of the plan. `scripts/verify.sh` runs its tests as
`SQLX_OFFLINE=true cargo test --workspace`, so this task's own acceptance line reads
the `.sqlx` cache — and Step 4 just changed the row shape every posts entry in it
describes. Without this the task cannot go green.

`prepare` rewrites the directory wholesale, so a partially-updated cache is not a
failure mode you have to reason about.

- [ ] **Step 6: Commit**

```bash
git add migrations/013_post_visibility.sql src/db.rs src/models.rs .sqlx
git commit -m "feat(artportfolio): the visibility column and its two parse modes"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

### Task 2: `Viewer` and the db-layer filter

**Class:** B

**Why this class:** Pure data-layer logic, and the tests below name every case and
its expected value. The gating these functions implement is reviewed where it is
actually wired to a session — Task 3.

**Files:**
- Modify: `src/models.rs` — `Viewer` enum, `PostCounts` struct
- Modify: `src/db.rs` — `get_posts_page`, `count_posts`, new `get_post_by_id`, `set_post_visibility`
- Modify: `src/routes/admin.rs:52` — the one non-feed caller
- Test: `src/db.rs` `mod tests`

**Interfaces:**
- Consumes: `Visibility` from Task 1.
- Produces:
  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum Viewer { Visitor, Admin }
  impl Viewer { pub fn is_admin(&self) -> bool; }

  pub struct PostCounts { pub total: i64, pub public: i64, pub unlisted: i64, pub hidden: i64 }

  pub async fn get_posts_page(pool: &DbPool, q: Option<&str>, page: i64, viewer: Viewer) -> Vec<Post>;
  pub async fn count_posts(pool: &DbPool, q: Option<&str>, viewer: Viewer) -> PostCounts;
  pub async fn get_post_by_id(pool: &DbPool, id: i64, viewer: Viewer) -> Option<Post>;
  pub async fn set_post_visibility(pool: &DbPool, id: i64, visibility: Visibility) -> bool;
  ```

- [ ] **Step 1: Add `Viewer` and `PostCounts` to `models.rs`**

```rust
/// Who is asking.
///
/// Built once at the handler edge from `OptionalAuth` and the `?visitor=1`
/// preview flag, then used for both the db call and the template flags. Deriving
/// them separately is how a preview ends up showing admin chrome over a
/// visitor's posts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Viewer {
    Visitor,
    Admin,
}

impl Viewer {
    pub fn is_admin(&self) -> bool {
        matches!(self, Self::Admin)
    }
}

/// The page head's numbers.
///
/// `total` is viewer-dependent: an admin's total is every post, a visitor's is
/// the public count. The other three are rendered for an admin only.
#[derive(Debug, Clone, Copy, Default)]
pub struct PostCounts {
    pub total: i64,
    pub public: i64,
    pub unlisted: i64,
    pub hidden: i64,
}
```

- [ ] **Step 2: Add the viewer filter to `get_posts_page`**

Keep the existing **two** branches (`q` / no-`q`) and bind the viewer as a bool:

```sql
WHERE (? OR visibility = 'public')
```

for the no-`q` branch, and

```sql
WHERE (? OR visibility = 'public') AND caption LIKE ? ESCAPE '\'
```

for the `q` branch. Bind `viewer.is_admin()` **first** in the `q` branch — argument
order follows placeholder order.

Do not expand this to four literal branches. The bool bind was verified to infer
before this plan was written: three candidate queries were compiled against a copy
of `portfolio.db` carrying migration 013 and all three passed. Document the choice
where the existing two-branch comment already explains itself, around `db.rs:120`:

```rust
/// The viewer is a bool bind rather than a third and fourth query branch. Four
/// near-identical SQL literals is the naive cost of crossing `q` with the
/// viewer, and this file already carries a comment explaining why it split into
/// two — that reasoning does not extend to doubling again.
///
/// The trade is an index: SQLite cannot use `posts(visibility, created_at DESC)`
/// against an OR-with-a-parameter predicate, so none is created. At a few
/// hundred posts the scan is free. **If this ever becomes four literal branches,
/// add the index in the same commit** — the visitor branch would then be a plain
/// `WHERE visibility = 'public' ORDER BY created_at DESC` and would use it
/// perfectly. The two decisions are one decision.
```

- [ ] **Step 3: Rewrite `count_posts` to return `PostCounts`**

One `GROUP BY` per branch — and note there is **no viewer branch in the SQL at all**.
The query counts every state; the viewer decides only what `total` means:

```rust
pub async fn count_posts(pool: &DbPool, q: Option<&str>, viewer: Viewer) -> PostCounts {
    struct Row {
        visibility: String,
        n: i64,
    }

    let rows: Vec<Row> = match q {
        Some(q) => {
            let pattern = like_pattern(q);
            sqlx::query_as!(
                Row,
                r#"SELECT visibility AS "visibility!: String", COUNT(*) AS "n: i64"
                   FROM posts WHERE caption LIKE ? ESCAPE '\' GROUP BY visibility"#,
                pattern
            )
            .fetch_all(pool)
            .await
            .unwrap_or_default()
        }
        None => sqlx::query_as!(
            Row,
            r#"SELECT visibility AS "visibility!: String", COUNT(*) AS "n: i64"
               FROM posts GROUP BY visibility"#
        )
        .fetch_all(pool)
        .await
        .unwrap_or_default(),
    };

    let mut counts = PostCounts::default();
    // GROUP BY returns no row for a state with zero posts, so accumulate into
    // defaults rather than indexing the result. A portfolio with nothing hidden
    // is the normal case, not an edge one.
    for row in rows {
        match Visibility::from_row(&row.visibility) {
            Visibility::Public => counts.public = row.n,
            Visibility::Unlisted => counts.unlisted = row.n,
            Visibility::Hidden => counts.hidden = row.n,
        }
    }
    counts.total = if viewer.is_admin() {
        counts.public + counts.unlisted + counts.hidden
    } else {
        counts.public
    };
    counts
}
```

The `AS "n: i64"` override is load-bearing for the same reason the old
`as "count: i64"` was — sqlx infers SQLite's `COUNT(*)` as `i32`. It is easy to lose
in the move from `query_scalar!` to `query_as!`.

- [ ] **Step 4: Add `get_post_by_id` and `set_post_visibility`**

```rust
/// One post, or `None` when this viewer may not have it.
///
/// A missing id and a hidden post are the same answer on purpose: the caller
/// turns both into a 404, so from outside they are indistinguishable. Returning
/// a distinguishable error would confirm the row exists.
pub async fn get_post_by_id(pool: &DbPool, id: i64, viewer: Viewer) -> Option<Post> {
    let post = sqlx::query_as!(
        Post,
        "SELECT id, caption, image_url, webp_url, avif_url, format, file_size_bytes, created_at, image_width, image_height, visibility FROM posts WHERE id = ?",
        id
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()?;

    match (Visibility::from_row(&post.visibility), viewer) {
        (Visibility::Hidden, Viewer::Visitor) => None,
        _ => Some(post),
    }
}
```

Note `unlisted` is deliberately **not** filtered here — reachable by permalink is the
whole point of the state.

`set_post_visibility` is a plain `UPDATE … WHERE id = ?` returning
`rows_affected() == 1`, so Task 5's route can answer 404 for an unknown id.

- [ ] **Step 5: Update the callers**

Four call sites, and **the value matters at each one**:

| Call site | Viewer |
|---|---|
| `feed.rs` × 3 | Task 3 wires these; leave them compiling with `Viewer::Visitor` for now and let Task 3 replace it |
| `admin.rs:52` (admin dashboard) | **`Viewer::Admin`** |
| `db.rs` tests | explicit per test |

`admin.rs:52` is the one that fails quietly. Pass `Visitor` there and the dashboard
still compiles, still renders, and simply stops listing the posts an admin most needs
to see. `test_admin_dashboard_query_sees_all_states` in Step 6 asserts against
exactly this.

**Say plainly what this task's commit leaves behind.** With `feed.rs` hardcoded to
`Viewer::Visitor`, the feed ignores sessions entirely — an admin sees a visitor's
posts. That is a deliberate intermediate that Task 3 closes inside this same plan,
but an implementer who stops here has shipped the mirror image of the leak the plan
exists to fix. Do not end a session on this commit.

- [ ] **Step 6: Write the tests**

In `src/db.rs`'s `mod tests`, using the existing `test_pool()` helper. Insert posts
and set their states with a direct `UPDATE`, since nothing writes visibility until
Task 5:

| Test | Setup | Assert |
|---|---|---|
| `test_get_posts_page_visitor_sees_only_public` | 1 public, 1 unlisted, 1 hidden | `len() == 1`, and its caption is the public one |
| `test_get_posts_page_admin_sees_all` | same | `len() == 3` |
| `test_get_posts_page_visitor_filter_applies_with_search` | public "cat", hidden "cat" | `q = Some("cat")`, visitor → `len() == 1` |
| `test_count_posts_visitor_total_is_public_only` | 1 public, 1 unlisted, 2 hidden | `total == 1`, `public == 1` |
| `test_count_posts_admin_total_is_everything` | same | `total == 4, public == 1, unlisted == 1, hidden == 2` |
| `test_count_posts_absent_states_are_zero` | 2 public only | `unlisted == 0 && hidden == 0` — the `GROUP BY` gap |
| `test_get_post_by_id_hidden_is_none_for_visitor` | 1 hidden | `None` for `Visitor`, `Some` for `Admin` |
| `test_get_post_by_id_unlisted_is_some_for_visitor` | 1 unlisted | `Some` for both |
| `test_get_post_by_id_unknown_id_is_none` | empty | `None` for `Admin` too |
| `test_visibility_from_row_fails_closed` | — | `Visibility::from_row("bogus") == Visibility::Hidden` |
| `test_visibility_from_str_rejects_unknown` | — | `Visibility::from_str("bogus").is_none()` |
| `test_set_post_visibility_round_trip` | 1 public | set to `Hidden`, re-read, assert the column changed |
| `test_set_post_visibility_unknown_id_is_false` | empty | returns `false` |
| `test_admin_dashboard_query_sees_all_states` | 1 of each | `get_posts_page(&pool, None, 0, Viewer::Admin).len() == 3` — catches `admin.rs:52` being handed `Visitor` |

- [ ] **Step 7: Regenerate the sqlx offline cache**

```bash
cargo sqlx prepare
```

Step 4 added two queries the cache has never seen, and this task's acceptance line
runs the tests offline. Same reasoning as Task 1, Step 5.

- [ ] **Step 8: Commit**

```bash
git add src/db.rs src/models.rs src/routes/admin.rs .sqlx
git commit -m "feat(artportfolio): every post read now says who is asking"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

### Task 3: The effective viewer through `feed.rs`

**Class:** C

**Why this class:** Session gating, and a cross-task invariant a test suite can only
partly reach. Two of the three feed routes do not currently extract `OptionalAuth` at
all, and the preview flag has to stay consistent across a pagination boundary that
lives in a generated URL. **This is the highest-risk task in the plan.**

**Files:**
- Modify: `src/routes/feed.rs` — `PageQuery`, `head_label`, `load_more_url`, `page_url`, `render_grid`, `feed_page`, `htmx_posts`, `api_posts`
- Test: `src/routes/feed.rs` `mod tests`

**Interfaces:**
- Consumes: `Viewer`, `PostCounts`, `get_posts_page`, `count_posts` from Task 2.
- Produces:
  ```rust
  pub struct PageQuery { pub page: Option<i64>, pub q: Option<String>,
                         pub last_month: Option<String>, pub visitor: Option<String> }
  fn is_preview(query: &PageQuery) -> bool;               // visitor.as_deref() == Some("1")
  fn effective_viewer(session_is_admin: bool, preview: bool) -> Viewer;
  fn head_label(counts: &PostCounts, q: Option<&str>, viewer: Viewer) -> String;
  fn load_more_url(next_page: i64, q: Option<&str>, last_month: Option<&str>, preview: bool) -> String;
  fn page_url(q: Option<&str>, preview: bool) -> String;
  async fn render_grid(state, page, q, last_month, head_label_oob, viewer: Viewer) -> String;
  ```

- [ ] **Step 1: `PageQuery` gains the preview flag, and two helpers appear**

```rust
/// `?visitor=1` — an admin asking to be shown a visitor's view.
///
/// A string rather than a bool: serde parses `Option<bool>` from `true`/`false`,
/// not from `1`, and the handoff specifies `visitor=1`.
pub visitor: Option<String>,
```

```rust
fn is_preview(query: &PageQuery) -> bool {
    query.visitor.as_deref() == Some("1")
}

/// The single viewer every other decision on the page is made from.
///
/// Deriving the db filter and the template flags separately is the failure this
/// function exists to prevent: under preview that combination renders a
/// visitor's post set with admin badges and controls over it, so the preview
/// gets wrong precisely the thing it exists to show.
fn effective_viewer(session_is_admin: bool, preview: bool) -> Viewer {
    if session_is_admin && !preview {
        Viewer::Admin
    } else {
        Viewer::Visitor
    }
}
```

- [ ] **Step 2: Thread the flag through both URL builders**

`load_more_url` and `page_url` each gain `preview: bool` and append
`s.append_pair("visitor", "1")` when it is set — alongside the existing `q` and
`last_month` pairs, using the same `form_urlencoded::Serializer`.

Without this, page 0 renders as a visitor and the first *Load more* renders as an
admin: hidden posts appear mid-feed, in the middle of the preview that exists to
prove they do not.

- [ ] **Step 3: `head_label` takes the counts and the viewer**

```rust
fn head_label(counts: &PostCounts, q: Option<&str>, viewer: Viewer) -> String
```

In this task the body renders `counts.total` where `total` used to be, and nothing
branches on `viewer` yet — `counts.total` already encodes it. Task 7 adds the admin
split. The parameter is threaded now so Task 7 changes one function body rather than
re-opening this signature and every call to it.

- [ ] **Step 4: `render_grid` takes the viewer**

Add `viewer: Viewer` as the last parameter and pass it to `get_posts_page`. Task 6
uses this same value for the card flag, so its signature does not move again.

- [ ] **Step 5: Wire all three routes**

`feed_page` already extracts `OptionalAuth(is_admin)`. Rename the binding to
`session_is_admin`, compute `preview` and `effective`, and pass `effective` to both
`render_grid` and `count_posts`.

`FeedTemplate.is_admin` is fed from **`effective.is_admin()`**, not the raw bool.
Under preview the `{% if is_admin %}` upload composer in `feed.html` therefore
disappears. That is correct — a visitor has no composer — and it will look like a
regression to anyone who has not read this line.

`FeedTemplate` also gains `is_previewing: bool`, fed from the *raw* session bool AND
the preview flag (`session_is_admin && preview`). This is the one thing the raw bool
is for: Task 7 renders the "exit preview" affordance from it, and a real visitor must
never see that control.

**`htmx_posts` and `api_posts` must now extract `OptionalAuth` too. They do not
today.** While every post is public that is harmless; the moment Task 1 lands it is a
live leak — page 0 renders filtered and the first *Load more* hands back everything.
Add the extractor as the first argument of each, matching `feed_page`'s order.

`api_posts` reads the session but **not** the preview flag: the JSON API has no head,
no pagination UI and nothing to preview.

- [ ] **Step 6: Write the tests**

In `feed.rs`'s `mod tests`. The existing `test_app()` helper builds an unauthenticated
router, so these cover the visitor side directly; the admin side needs a session row
and a `Cookie: session=…` header, following `admin.rs`'s test setup.

| Test | Assert |
|---|---|
| `test_feed_page_visitor_omits_hidden` | body contains the public caption, not the hidden one |
| `test_htmx_posts_visitor_omits_hidden_page_0` | same, via `/artportfolio/htmx/posts?page=0` |
| `test_htmx_posts_visitor_omits_hidden_page_1` | **the leak this task exists to close** — insert 25 posts with a hidden one on page 1, assert it is absent |
| `test_api_posts_visitor_omits_hidden` | parse the JSON, assert no hidden id |
| `test_api_posts_visitor_omits_unlisted` | unlisted is out of the API too, not only the feed |
| `test_load_more_url_carries_visitor_flag` | `load_more_url(1, None, None, true)` contains `visitor=1` |
| `test_page_url_carries_visitor_flag` | `page_url(Some("cat"), true)` contains both `q=cat` and `visitor=1` |
| `test_effective_viewer_preview_downgrades_admin` | `effective_viewer(true, true) == Viewer::Visitor` |
| `test_effective_viewer_preview_flag_cannot_promote` | `effective_viewer(false, false) == Viewer::Visitor` — a visitor passing `?visitor=0` gains nothing |

- [ ] **Step 7: Commit**

```bash
git add src/routes/feed.rs
git commit -m "fix(artportfolio): two feed routes never asked who was calling"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

### Task 4: The permalink route

**Class:** C

**Why this class:** Session gating again, and the 404-not-403 distinction is a
security property rather than a behaviour a type checker can hold.

**Files:**
- Create: `templates/artportfolio/post.html`
- Modify: `src/routes/feed.rs` — new handler and route registration
- Modify: `static/style.css` — a section under `body.art-page`
- Modify: `templates/partials/post_card.html` — the card links to its permalink
- Test: `src/routes/feed.rs` `mod tests`

**Interfaces:**
- Consumes: `get_post_by_id`, `Viewer`, `effective_viewer`, `is_preview`.
- Produces: `GET /artportfolio/{id}`.

- [ ] **Step 1: The template**

`templates/artportfolio/post.html` extends `base.html` — required of every
user-facing page, and never with a `<style>` block. It renders the drawing at full
width with its caption, its date and a link back to `/artportfolio`, reusing the
`hm-*` primitives slice 1 established. Follow `templates/artportfolio/feed.html` for
the page shell and `partials/post_card.html` for the `<picture>` element's variant
fallbacks — the empty-`avif_url` branch is load-bearing here too, since a freshly
uploaded post genuinely has none for a second or two.

- [ ] **Step 2: The handler**

```rust
async fn post_permalink(
    OptionalAuth(session_is_admin): OptionalAuth,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Query(query): Query<PageQuery>,
) -> Response
```

`Path` is not currently imported in `feed.rs` — add it to the existing
`axum::extract::{Query, State}` line.

Derive `effective` exactly as Task 3 does — a previewing admin gets the visitor's
404 on a hidden post, or the preview lies about the one thing it exists to show.

`get_post_by_id` returning `None` becomes `StatusCode::NOT_FOUND` with a minimal
page. A missing id and a hidden post produce **the same response**, byte for byte.

- [ ] **Step 3: Register the route**

```rust
.route("/artportfolio/{id}", get(post_permalink))
```

No collision with the existing routes: `/artportfolio/htmx/posts` and
`/artportfolio/api/posts` are two segments deep, this is one. Register it last
anyway, so the file reads in specificity order.

- [ ] **Step 4: Link cards to their permalink**

In `post_card.html`, wrap the `<picture>` in an anchor to `/artportfolio/{{ post.id }}`.
Keep `tabindex="-1"` on the article: slice 1 chose it so J/K navigation does not put
twenty cards into the Tab order, and an anchor inside would undo that if it became
focusable in the same pass — give the anchor `tabindex="-1"` as well.

- [ ] **Step 5: Write the tests**

| Test | Assert |
|---|---|
| `test_permalink_public_is_200_for_visitor` | 200 |
| `test_permalink_unlisted_is_200_for_visitor` | 200 — the state's entire reason to exist |
| `test_permalink_hidden_is_404_for_visitor` | 404 |
| `test_permalink_hidden_is_200_for_admin` | 200 with a session cookie |
| `test_permalink_unknown_id_is_404` | 404, and the body is byte-identical to the hidden case |
| `test_permalink_hidden_is_404_for_previewing_admin` | session cookie **plus** `?visitor=1` → 404 |

- [ ] **Step 6: Commit**

```bash
git add templates/artportfolio/post.html templates/partials/post_card.html src/routes/feed.rs static/style.css
git commit -m "feat(artportfolio): unlisted posts get somewhere to be reached"
```

**Acceptance:** `./scripts/verify.sh` — all green.

> **Session boundary, if context is running long.** Tasks 1–4 leave the model
> complete and enforced — deployable, with the honest limitation that nothing in the
> UI can *change* a state yet. This is the one clean place to stop and resume from
> the ledger. Tasks 5–8 do not leave a comparable boundary.

---

### Task 5: Changing a visibility — the `PATCH` route and the upload field

**Class:** C

**Why this class:** A state-mutating admin route. `AuthSession` gating is the whole
protection here: a visitor who can reach this endpoint can unhide anything.

**Files:**
- Modify: `src/routes/admin.rs` — new handler, route registration, upload handler
- Modify: `src/db.rs` — `insert_post` gains the visibility parameter
- Modify: `src/main.rs` — route registration if not nested under the admin router
- Test: `src/routes/admin.rs` `mod tests`

**Interfaces:**
- Consumes: `set_post_visibility`, `Visibility::from_str`, `PostCardTemplate`.
- Produces:
  ```rust
  // PATCH /api/admin/posts/{id}/visibility
  // Body: form or JSON carrying `visibility`
  pub async fn insert_post(…, visibility: Visibility) -> Post;   // new trailing parameter
  ```

- [ ] **Step 1: The `PATCH` handler**

Gated by `AuthSession`, not `OptionalAuth` — this one mutates. Follow the existing
`DELETE /api/admin/posts/{id}` handler in the same file for extractor order and error
shape.

Three outcomes, and they are distinct on purpose:

| Input | Response |
|---|---|
| unrecognised visibility string | **400** — `Visibility::from_str` returned `None`; fail loudly, per Global Constraints |
| unknown post id | **404** — `set_post_visibility` returned `false` |
| success | **200** with the re-rendered card |

The success body is a `PostCardTemplate` with `is_admin: true` — an admin is the only
caller — swapped `outerHTML` into `closest .hm-post` by the button's HTMX attributes
(Task 6). Returning the card rather than a status keeps the badge and the opacity in
step with the database without a reload.

- [ ] **Step 2: The upload gains a `visibility` field**

`insert_post` takes `visibility: Visibility` as a trailing parameter and writes
`visibility.as_str()`. Its SQL changes, which is why this task regenerates the cache.

In the multipart loop, read an optional `visibility` field. **Absent means `public`**
(`Visibility::default()`); **present but unrecognised means 400**, exactly as the
`PATCH` body does. An absent field is the normal case — Task 6 does not add an upload
control, and slice 4's tray is what will send this.

Update `insert_post`'s existing test callers to pass `Visibility::Public`.

- [ ] **Step 3: Write the tests**

| Test | Assert |
|---|---|
| `test_patch_visibility_requires_session` | no cookie → redirect or 401, **never** 200 |
| `test_patch_visibility_sets_state` | with a session, `public` → `hidden`, re-read the row |
| `test_patch_visibility_unknown_string_is_400` | body `visibility=bogus` → 400, row unchanged |
| `test_patch_visibility_unknown_id_is_404` | 404 |
| `test_patch_visibility_returns_card_markup` | body contains `hm-post` and the badge class |
| `test_upload_absent_visibility_defaults_public` | `insert_post` with `Visibility::default()` stores `"public"` |
| `test_upload_unknown_visibility_is_400` | multipart carrying `visibility=bogus` → 400 |

- [ ] **Step 4: Regenerate the sqlx offline cache**

```bash
cargo sqlx prepare
```

`insert_post`'s SQL changed in Step 2.

- [ ] **Step 5: Commit**

```bash
git add src/routes/admin.rs src/db.rs src/main.rs .sqlx
git commit -m "feat(artportfolio): visibility becomes something you can change"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

### Task 6: The card badge, the dimmed hidden card, and the control cluster

**Class:** A

**Why this class:** Askama templates compile into the binary, so broken markup is a
build error, and `tests/static_assets.rs` catches the nested-comment CSS failure that
has bitten this repo before. Everything here is compiler- or lint-gated.

**Files:**
- Modify: `templates/partials/post_card.html`
- Modify: `src/routes/feed.rs` — `PostCardTemplate`, `PostGridTemplate`, `render_grid`
- Modify: `src/routes/admin.rs` — the two `PostCardTemplate` construction sites
- Modify: `static/style.css` — under the existing `body.art-page` section

**Interfaces:**
- Consumes: `Viewer` from Task 2, `render_grid`'s `viewer` parameter from Task 3.
- Produces: `PostCardTemplate { post, is_first, is_admin }`, `PostGridTemplate { …, is_admin }`.

- [ ] **Step 1: Thread the flag to the card — through the grid, not around it**

`post_card.html` is **`include`d** from `post_grid.html` with `{% let %}` bindings, so
adding `is_admin` to `PostCardTemplate` alone leaves every card in the feed unable to
see it — only the upload response would get it. Both templates need the field:

- `PostCardTemplate` gains `is_admin: bool`
- `PostGridTemplate` gains `is_admin: bool`
- `render_grid` sets it from the `viewer` it already takes: `is_admin: viewer.is_admin()`

One value, two uses, no signature churn. `admin.rs`'s two construction sites pass
`true`.

- [ ] **Step 2: The card markup**

Everything below lives behind `{% if is_admin %}`. **A visitor's card must stay
byte-identical to slice 1's** — that is the check for this step.

- A visibility badge at top-right, 8px inset, its class derived from the state.
- The control cluster at top-left: `eye-off` hide and `link` unlist, 28px
  IconButtons, 6px gap, faded in on card hover and on keyboard focus within the card.
  Each is an HTMX `hx-patch` to `/api/admin/posts/{{ post.id }}/visibility` with
  `hx-target="closest .hm-post" hx-swap="outerHTML"`.
- Edit, collection and delete controls from the handoff's cluster belong to slices
  3–5. Do not add them.

- [ ] **Step 3: The styles**

Under the existing `body.art-page` section in `style.css`, in a named subsection
comment. Never a `<style>` block in a template that extends `base.html`.

```css
/* Visibility badge — tones from the handoff, S2. */
--art-vis-public:   #4FD6A8;   /* mint    */
--art-vis-unlisted: #FFB570;   /* amber   */
--art-vis-hidden:   inherit;   /* neutral */
```

A hidden card renders at `opacity: .5`. The control cluster transitions over 130ms
`cubic-bezier(.2,.8,.3,1)`, and `prefers-reduced-motion: reduce` zeroes it.

Under 900px the cluster is always visible at 44px hit targets rather than
hover-revealed — a touch device has no hover, so a hover-only control is no control.

**Do not nest `/* */` inside these comments.** `tests/static_assets.rs` exists because
a nested marker silently dropped the next rule once; browsers resolve it by
discarding, not erroring.

- [ ] **Step 4: Commit**

```bash
git add templates/partials/post_card.html src/routes/feed.rs src/routes/admin.rs static/style.css
git commit -m "feat(artportfolio): a card that says what state it is in"
```

**Acceptance:** `./scripts/verify.sh` — all green.

**Browser checkpoint 1.** `cargo run`, then logged out: the feed omits hidden and
unlisted posts, and no badge or cluster appears anywhere. Logged in: all three states
appear, each badge matches its state, a hidden card is visibly dimmed, and clicking
hide swaps that one card without touching the rest of the page.

Record what you could not verify. Slice 1's checkpoints found `resize_window` never
changes `innerWidth` here, key events arrive synthetic rather than natively
delivered, and Dark Reader repaints colour — so **the three badge tones need human
eyes**, and the 900px always-visible cluster is only checkable through the CSSOM.

---

### Task 7: The split head counts and the view-as-visitor control

**Class:** B

**Why this class:** `head_label` is a pure function with four specified output
shapes, and the table below names every one with its expected string. The button and
the key handler are markup and a listener beside two that already exist.

**Files:**
- Modify: `src/routes/feed.rs` — `head_label`
- Modify: `templates/artportfolio/feed.html` — head actions
- Modify: `static/artfeed.js` — the `V` shortcut
- Modify: `static/palette.js` — the COMMANDS array
- Test: `src/routes/feed.rs` `mod tests`

**Interfaces:**
- Consumes: `PostCounts`, `Viewer`, `FeedTemplate.is_previewing` from Task 3.

- [ ] **Step 1: `head_label` gains the admin split**

Four shapes, exactly:

| | no search | searching |
|---|---|---|
| **visitor** | `117 drawings · newest first` | `12 drawings · matching "cat"` |
| **admin** | `128 drawings · 117 public · 4 unlisted · 7 hidden` | `12 drawings · matching "cat" · 9 public · 2 unlisted · 1 hidden` |

The split **replaces** the `· newest first` suffix and **follows** `· matching "…"`
when there is a search. Sort order is stated by the toolbar, so spending head room on
it while withholding the counts is the wrong trade.

The singular/plural on `drawing`/`drawings` follows `counts.total`, as it does today.
The three split numbers carry no noun.

- [ ] **Step 2: The head actions**

In `feed.html`, behind `{% if is_admin %}`, a secondary **View as visitor** button
(`eye` glyph) linking to `/artportfolio?visitor=1` — a plain link, so it is
bookmarkable and survives a reload.

Behind `{% if is_previewing %}` — the flag Task 3 fed from the *raw* session bool —
an "exit preview" control linking back to `/artportfolio`. These two are mutually
exclusive by construction: under preview `is_admin` is false, so only the exit
control renders.

- [ ] **Step 3: The `V` shortcut**

In `artfeed.js`'s existing `keydown` listener, after the `j`/`k` branch. It sits
**after** the `artfeedIsTyping(active)` guard already in place, so it cannot fire
while the search field has focus.

```js
    } else if (e.key === 'v' || e.key === 'V') {
      // Admin-only in effect: a visitor's page has no such link, so this
      // resolves to null and the key does nothing.
      const toggle = document.getElementById('art-visitor-toggle');
      if (toggle) {
        e.preventDefault();
        toggle.click();
      }
    }
```

No new listener and no new init path — `artfeedInit` is already idempotent via
`window.artfeedBound` and already bound on both `DOMContentLoaded` and
`htmx:afterSwap`, which is what the hx-boost rule requires.

- [ ] **Step 4: The palette entry**

Add "Toggle visitor view" to the `COMMANDS` array in `palette.js`, following the
existing entry shape exactly. **A nested object literal broke this file once**
(`c72d614`) and `node --check` in `verify.sh` is the only thing that catches it.

- [ ] **Step 5: Write the tests**

| Test | Assert |
|---|---|
| `test_head_label_visitor_no_search` | `"117 drawings · newest first"` |
| `test_head_label_visitor_with_search` | `"12 drawings · matching \"cat\""` |
| `test_head_label_admin_no_search` | `"128 drawings · 117 public · 4 unlisted · 7 hidden"` |
| `test_head_label_admin_with_search` | `"12 drawings · matching \"cat\" · 9 public · 2 unlisted · 1 hidden"` |
| `test_head_label_singular` | `total == 1` → `"1 drawing · …"` |
| `test_head_label_admin_zero_states_still_render` | nothing hidden → `"· 0 hidden"`, not an omitted clause |

- [ ] **Step 6: Commit**

```bash
git add src/routes/feed.rs templates/artportfolio/feed.html static/artfeed.js static/palette.js
git commit -m "feat(artportfolio): a head that counts each state, and a way to preview"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

### Task 8: Offline build, docs and the plan's final verification

**Class:** A

**Why this class:** Mechanical. The offline build either succeeds or it does not, and
the docs are prose.

**Files:**
- Modify: `CLAUDE.md` — migration list, route list, test counts
- Modify: `docs/WORKTREES.md` — the artportfolio card

- [ ] **Step 1: Confirm the offline build**

Tasks 1, 2 and 5 already regenerated `.sqlx` — they had to, since every acceptance
line runs the tests offline. This step confirms the end state is coherent, which is
what CI will do:

```bash
SQLX_OFFLINE=true cargo build --release
```

If this fails, something was committed between a query change and its `prepare`. Run
`cargo sqlx prepare` once more and commit the diff; a non-empty diff here means an
earlier task went green against a cache it had updated but not staged.

- [ ] **Step 2: Update `CLAUDE.md`**

Four places: the migration list gains "013 `posts.visibility`"; the route list gains
`GET /artportfolio/{id}` and `PATCH /api/admin/posts/{id}/visibility`; the models
paragraph gains `Visibility` and `Viewer`; the test counts move. **Re-measure the
counts** —

```bash
cargo test --workspace 2>&1 | grep "test result"
```

— never copy a number out of another document. `docs/WORKTREES.md` already warns that
two branches now edit this same line and whichever merges second must resolve it by
re-measuring.

- [ ] **Step 3: Update `docs/WORKTREES.md`**

Set the artportfolio card's Status to slice 2 complete and Next to slice 3
(collections, tags, the full filter rail — `superpowers:brainstorming`, then a spec).

Record the two trade-offs the spec accepted, so slice 3 does not rediscover them as
bugs: **unlisted posts are enumerable by sequential id**, and **`/admin` shows no
visibility badge** because it still renders through `admin_post_card_html()`, the
legacy format string.

Land this on `master` as well as the branch. That file's own header exists because an
orientation doc written only onto a feature branch becomes invisible from everywhere
else, and this stream has already done it twice.

- [ ] **Step 4: Commit**

```bash
git add CLAUDE.md docs/WORKTREES.md
git commit -m "docs: slice 2 lands — the visibility model"
```

**Acceptance:** `./scripts/verify.sh` — all green, plus
`SQLX_OFFLINE=true cargo build --release` succeeding.

---

## Before the plan is done

**Browser checkpoint 2** — the whole slice end to end, with at least one post in each
state:

1. Logged out: feed omits hidden and unlisted; the unlisted permalink renders; the
   hidden permalink 404s.
2. Logged in: all three in the feed, badges correct, head reads
   `N drawings · N public · N unlisted · N hidden`.
3. Click hide on a card — the badge and dimming change without a reload, and the head
   goes stale by one until the next load. **That staleness is expected**: it is the
   same debt slice 1 recorded for the composer, and slice 4 owns the fix.
4. Press `V`, then click *Load more* **before doing anything else**. This is the click
   that catches a missing `visitor=1` in `load_more_url`.
5. Under preview: no badges, no cluster, no composer, and the hidden permalink 404s.

Then **one review of the whole plan's diff** on the most capable model — Class A and
B tasks get no per-task reviewer under `plan-economics`, so this pass is their only
review. Point it at the state table in Global Constraints and ask specifically
whether any route can be made to return a post the table says it may not.

Four places where the code compiles, the tests pass, and the behaviour is still
wrong — name these for the reviewer:

1. **`admin.rs:52`** — passing `Viewer::Visitor` empties the admin dashboard silently.
2. **The URL builders** — a missing `visitor=1` only shows up after a *Load more*.
3. **`FeedTemplate.is_admin`** — fed from the raw session bool instead of `effective`,
   the preview renders admin chrome over a visitor's posts.
4. **`PostGridTemplate.is_admin`** — added to `PostCardTemplate` only, every card in
   the feed loses its badge while the upload response keeps one, which looks like a
   caching bug rather than a missing field.
