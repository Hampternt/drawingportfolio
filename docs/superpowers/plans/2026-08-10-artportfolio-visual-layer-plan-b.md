# /artportfolio visual layer — Plan B (of 2) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILLS: `plan-economics` (this repo's task
> classes, review policy and plan sizing), then superpowers:executing-plans to run it
> task by task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give `/artportfolio` caption search, month grouping and a keyboard layer —
the behavioural half of slice 1, on top of the visual layer Plan A shipped.

**Architecture:** The database gains a filtered page query and a real COUNT; the
handler gains all the derived shape (normalised query, month groups, the Load more
URL, the page-head label), so the templates stay logic-free. Month grouping forces
one CSS change that is not cosmetic: `columns` moves off `#feed` and onto a per-month
grid, because a full-bleed divider cannot live inside a multi-column flow.

**Slice:** When this plan is done, slice 1 is complete and deployable: the feed is
searchable by caption, grouped by month with dividers that survive pagination, has a
sticky filter rail with a keyboard legend, and answers to `/`, `Esc`, `J` and `K`.
Slices 2–5 (visibility model, collections + tags, multi-upload tray, select mode)
remain untouched.

Source spec: `docs/superpowers/specs/2026-08-09-artportfolio-visual-layer-design.md`.
**The spec carries six documented factual errors about this codebase** — five recorded
in Plan A's "Spec corrections", the sixth in this plan's. Trust the code over the spec.

Design authority: `docs/design/artportfolio-redesign/README.md`.

**Baseline:** `e67cf49`, `./scripts/verify.sh` green, 238 tests.

## Global Constraints

- **Blast radius.** `/`, `/tasks`, `/fitness`, `/admin` and `/drinks` render exactly as
  they do today. Every painting rule stays under `body.art-page`.
- **`./scripts/verify.sh` is the only gate.** Never accept bare `cargo test`: the root
  `Cargo.toml` is both a package and the workspace root, so `cargo test` runs 61 of 238
  and silently skips `drinkinggame`'s 177.
- **`cargo sqlx prepare` runs once, in Task 1.** No schema change in this plan — but
  `.sqlx` filenames are content-addressed by *query text*, so a renamed function with
  new SQL needs new cache entries. `verify.sh` runs the suite under `SQLX_OFFLINE=true`,
  so a missing entry fails the gate for every later task. Expect **delete + add**, never
  modify.
- **No `<style>` blocks** in templates extending `base.html`. CSS goes in
  `static/style.css`, inside the existing `/* ── Drawing Portfolio ── */` section.
- **No nested `/* */` in `static/style.css`** — `tests/static_assets.rs` fails the build
  on it, because browsers silently drop the rule after the stray marker.
- **Both shells.** Any global script or style lands in **both** `templates/base.html`
  and `templates/admin.html` (`admin.html` is standalone by recorded exception).
- **SQL lives in `db.rs` only.** Route handlers call db functions; templates receive
  pre-computed values and contain no logic beyond simple conditionals.
- **The `{% if is_admin %}` composer in `feed.html` survives untouched** — markup,
  inline `<script>`, and its `hx-target="#feed" hx-swap="afterbegin"` contract. The
  multi-upload tray that replaces it is slice 4.
- **`PostCardTemplate` has three callers** — the inlined first page, the HTMX route, and
  `admin.rs`'s upload response behind `source == "gallery"`. Any change to
  `templates/partials/post_card.html` changes all three.
- **Askama escapes to numeric character references** (`&#60;`), not named (`&lt;`).
  Assert the property, never the spelling.
- **Askama's `{% include %}` inlines against the caller's scope** — loop variables and
  `{% let %}` bindings are visible inside the included template. That is how
  `post_grid.html` drives the card.
- **Copy is fixed.** Page head reads `117 drawings · newest first`, and under an active
  search `12 drawings · matching "loomis"`. Month divider reads `2026-07 · 5 drawings`.
- **Chrono stays pinned to `0.4.34`.** Nothing in this plan needs it.

**Verification for every task:** `./scripts/verify.sh` — all green, output quoted in the
report.

**Browser checkpoint:** after Task 5 only. Not per task.

---

### Task 1: `get_posts_page`, `count_posts`, and LIKE escaping

**Class:** B (logic whose tests are written below)

**Why this class:** Every case has a named expected value in Step 5, and the offline
cache — the one thing that could rot silently — is decided by `verify.sh` itself, which
compiles the whole workspace under `SQLX_OFFLINE=true`. Nothing left for a reviewer.

**Files:**
- Modify: `src/db.rs` — `get_posts` (line ~97) renamed and widened, `count_posts` added,
  five test call sites updated
- Modify: `src/routes/admin.rs:52` — dashboard call site
- Modify: `src/routes/feed.rs:76,131,213` — two handler call sites and one test
- Modify: `.sqlx/` (regenerated)

**Interfaces:**
- Consumes: nothing.
- Produces:
  ```rust
  pub fn like_pattern(q: &str) -> String;
  pub async fn get_posts_page(pool: &DbPool, q: Option<&str>, page: i64) -> Vec<Post>;
  pub async fn count_posts(pool: &DbPool, q: Option<&str>) -> i64;
  ```
  `get_posts` ceases to exist. `like_pattern` is `pub` only so its escaping can be
  tested directly.

- [ ] **Step 1: Write the escaping helper**

In `src/db.rs`, above `get_posts_page`:

```rust
/// Builds the `LIKE` pattern for a caption search.
///
/// Escapes the escape character first, then LIKE's two wildcards, then wraps the
/// result in `%…%`. The order is not a style choice: escaping `%` before `\`
/// would send the second pass back over the backslashes the first one just
/// introduced and double them.
///
/// Without this, a search for `100%` becomes the pattern `%100%%` and matches
/// every row in the table.
pub fn like_pattern(q: &str) -> String {
    let escaped = q
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    format!("%{escaped}%")
}
```

- [ ] **Step 2: Rename `get_posts` to `get_posts_page` and add the filter**

Two `query_as!` branches, not one query with `?1 IS NULL`: sqlx's SQLite macro does not
reliably support reusing a numbered placeholder, and a two-branch match needs no
argument about NULL semantics at all.

```rust
pub async fn get_posts_page(pool: &DbPool, q: Option<&str>, page: i64) -> Vec<Post> {
    let offset = page * 20;
    match q {
        Some(q) => {
            let pattern = like_pattern(q);
            sqlx::query_as!(Post,
                "SELECT id, caption, image_url, webp_url, avif_url, format, file_size_bytes, created_at, image_width, image_height FROM posts WHERE caption LIKE ? ESCAPE '\\' ORDER BY created_at DESC LIMIT 21 OFFSET ?",
                pattern, offset
            )
            .fetch_all(pool)
            .await
            .unwrap_or_default()
        }
        None => { /* the existing unfiltered query, unchanged */ }
    }
}
```

`ESCAPE '\\'` in a Rust string literal is the SQL text `ESCAPE '\'`. Keep the N+1 probe:
`LIMIT 21` with `OFFSET page * 20` is what answers "is there another page?" without a
COUNT, and nothing in this plan replaces it. SQLite's `LIKE` is ASCII-case-insensitive
by default, which is the behaviour the spec asks for — do not add `COLLATE NOCASE`.

- [ ] **Step 3: Add `count_posts`**

```rust
/// The real total for the page head. Called only on a full page render — never
/// on HTMX pagination, where the head is not re-rendered and the COUNT would be
/// wasted work on every Load more.
pub async fn count_posts(pool: &DbPool, q: Option<&str>) -> i64 {
    match q {
        Some(q) => {
            let pattern = like_pattern(q);
            sqlx::query_scalar!(
                r#"SELECT COUNT(*) AS "count: i64" FROM posts WHERE caption LIKE ? ESCAPE '\'"#,
                pattern
            )
            .fetch_one(pool)
            .await
            .unwrap_or(0)
        }
        None => { /* same shape, no WHERE */ }
    }
}
```

The `AS "count: i64"` override is load-bearing: sqlx infers SQLite's `COUNT(*)` as `i32`,
and the signature is `i64`. Note the raw string literal — inside `r#"…"#` the escape
character is written once, as `ESCAPE '\'`.

- [ ] **Step 4: Fix every call site**

The compiler enumerates them; there are eight.

| Site | Change |
|---|---|
| `src/routes/admin.rs:52` | `get_posts_page(&state.pool, None, 0)` — dashboard behaviour unchanged |
| `src/routes/feed.rs:76` | inside `render_page`, `None` for now (Task 2 threads `q` through) |
| `src/routes/feed.rs:131` | inside `api_posts`, `None` for now (Task 2 adds the filter) |
| `src/routes/feed.rs:213` | test |
| `src/db.rs` ×5 (lines ~1017, 1043, 1126, 1147, 1170) | tests |

- [ ] **Step 5: Write the tests**

In `src/db.rs`'s test module. Five cases, with their expected values:

```rust
#[test]
fn test_like_pattern_escapes_wildcards() {
    assert_eq!(like_pattern("100%"), "%100\\%%");
    assert_eq!(like_pattern("a_b"), "%a\\_b%");
    // The escape character itself is doubled first, so it survives as a literal.
    assert_eq!(like_pattern("c:\\x"), "%c:\\\\x%");
    assert_eq!(like_pattern("loomis"), "%loomis%");
}

#[tokio::test]
async fn test_get_posts_page_unfiltered_keeps_the_n_plus_1_probe() {
    // 21 rows: page 0 returns all 21 (the 21st is the has_more probe, dropped
    // by the caller), page 1 returns the single leftover.
    assert_eq!(get_posts_page(&pool, None, 0).await.len(), 21);
    assert_eq!(get_posts_page(&pool, None, 1).await.len(), 1);
}

#[tokio::test]
async fn test_get_posts_page_filters_captions_case_insensitively() {
    // captions: "Loomis head", "figure drawing"
    let hits = get_posts_page(&pool, Some("loomis"), 0).await;
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].caption, "Loomis head");
}

#[tokio::test]
async fn test_search_for_a_literal_percent_does_not_match_every_row() {
    // captions: "100% cotton paper", "graphite study"
    // Unescaped, the pattern would be %100%% — which matches both rows.
    let hits = get_posts_page(&pool, Some("100%"), 0).await;
    assert_eq!(hits.len(), 1, "a literal % must not act as a wildcard");
    assert_eq!(hits[0].caption, "100% cotton paper");
}

#[tokio::test]
async fn test_count_posts_agrees_with_the_filtered_result() {
    // captions: "gesture study", "hand study", "colour thumbnail"
    assert_eq!(count_posts(&pool, None).await, 3);
    assert_eq!(count_posts(&pool, Some("study")).await, 2);
    assert_eq!(count_posts(&pool, Some("nothing here")).await, 0);
}
```

Follow the existing test pattern in `src/db.rs`: in-memory pool, `run_migrations`, then
`insert_post(&pool, caption, "u", "", "", "single", 0, 0, 0)`.

- [ ] **Step 6: Regenerate the offline cache**

```bash
DATABASE_URL=sqlite:portfolio.db cargo sqlx prepare
git status --short .sqlx/
```

The worktree's `portfolio.db` was migrated through 012 during Plan A, so this runs
clean. Expect the old `get_posts` SELECT entry **deleted** and three added (filtered
SELECT, filtered COUNT, unfiltered COUNT) — content-addressed filenames mean a changed
query renames its file, so ` M` lines would mean something went wrong, not right.

An empty `.sqlx/` diff means the command did not actually run.

- [ ] **Step 7: Commit**

```bash
git add src/db.rs src/routes/admin.rs src/routes/feed.rs .sqlx
git commit -m "feat(artportfolio): caption search in the db layer"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

### Task 2: Month grouping, query plumbing and the real page-head count

**Class:** B (logic whose tests are written below)

**Why this class:** Grouping, `last_month` suppression, the label and the Load more URL
are pure functions over data the plan names, with expected values below. The one runtime
behaviour — the `HX-Push-Url` header — is asserted by an integration test against the
router.

**Files:**
- Modify: `src/models.rs` — add `MonthGroup`
- Modify: `src/routes/feed.rs` — `PageQuery`, `normalize_q`, `group_by_month`,
  `head_label`, `load_more_url`, `PostGridTemplate`, `render_grid`, `feed_page`,
  `htmx_posts`, `api_posts`, tests
- Modify: `templates/artportfolio/partials/post_grid.html`
- Modify: `templates/artportfolio/partials/empty_state.html`

**Interfaces:**
- Consumes: `db::get_posts_page(pool, q, page)`, `db::count_posts(pool, q)` from Task 1.
- Produces:
  ```rust
  // src/models.rs
  pub struct MonthGroup {
      pub label: String,       // "2026-07"
      pub count: usize,        // posts in THIS page's slice of the month
      pub show_divider: bool,  // false when last_month already rendered it
      pub posts: Vec<Post>,
  }

  // src/routes/feed.rs
  pub struct PageQuery { pub page: Option<i64>, pub q: Option<String>, pub last_month: Option<String> }
  fn normalize_q(raw: Option<&str>) -> Option<String>;
  fn group_by_month(posts: Vec<Post>, last_month: Option<&str>) -> Vec<MonthGroup>;
  fn head_label(total: i64, q: Option<&str>) -> String;
  fn load_more_url(next_page: i64, q: Option<&str>, last_month: Option<&str>) -> String;

  struct PostGridTemplate {
      groups: Vec<MonthGroup>,
      has_more: bool,
      load_more_url: String,
      is_first_page: bool,
      q: String,               // "" when no search — drives the filtered empty state
  }
  ```
  Task 3 additionally adds `q: String` to `FeedTemplate`.

- [ ] **Step 1: `MonthGroup`**

Added to `src/models.rs`. It needs no `Serialize` — it never crosses the API boundary.

`show_divider` is an extension over the spec's three fields, and it is the right shape:
suppression must not lose the label, because the group's label is what the *next* page's
`last_month` parameter is built from.

Document on `count` that it is this page's slice of the month, not the month's total. A
month spanning a page boundary leaves a divider reading `5 drawings` above 8 cards after
Load more, because the divider is not re-rendered when the continuation appends. That is
inherent to append-only pagination; a per-month COUNT is not in this slice's scope.

- [ ] **Step 2: `normalize_q` — one place, three bugs**

```rust
/// Trims the raw query and treats blank as absent.
///
/// One normalisation at the handler edge kills three separate defects: a head
/// label reading `matching ""`, a `%%` pattern that matches everything, and a
/// pushed URL carrying a pointless `?q=`.
fn normalize_q(raw: Option<&str>) -> Option<String> {
    raw.map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}
```

Every handler calls this once and passes `q.as_deref()` downwards.

- [ ] **Step 3: `group_by_month`**

`created_at` is ISO8601 `TEXT`, so `created_at[..7]` is `YYYY-MM`. Rows arrive already
sorted `created_at DESC`, so grouping is a single pass over consecutive runs — do not
sort or use a hash map, either would risk reordering the feed.

Use `post.created_at.get(..7).unwrap_or("")` rather than slicing: a malformed short
timestamp would panic on `[..7]`.

`show_divider` is `false` for the first group when its label equals `last_month`, and
`true` everywhere else.

- [ ] **Step 4: `head_label` — the real total replaces the honest fallback**

```rust
fn head_label(total: i64, q: Option<&str>) -> String {
    let noun = if total == 1 { "drawing" } else { "drawings" };
    match q {
        Some(q) => format!("{total} {noun} · matching \"{q}\""),
        None => format!("{total} {noun} · newest first"),
    }
}
```

The template renders it through `{{ head_label }}`, so Askama escapes a caption-hostile
query; do not pre-escape here.

**`test_head_label_states_a_total_only_when_it_knows_one` must be rewritten in this
step, not treated as a regression.** It pins the `"newest first"`-with-no-count fallback
that only existed because Plan A had no COUNT. Replace it with:

```rust
assert_eq!(head_label(117, None), "117 drawings · newest first");
assert_eq!(head_label(1, None), "1 drawing · newest first");
assert_eq!(head_label(0, None), "0 drawings · newest first");
assert_eq!(head_label(12, Some("loomis")), "12 drawings · matching \"loomis\"");
```

- [ ] **Step 5: `load_more_url`**

Built in Rust, not the template: Askama escapes HTML, not URLs, and a query containing
`&` or `%` must be percent-encoded before it lands in an attribute.

```rust
fn load_more_url(next_page: i64, q: Option<&str>, last_month: Option<&str>) -> String {
    let mut s = url::form_urlencoded::Serializer::new(String::new());
    s.append_pair("page", &next_page.to_string());
    if let Some(q) = q { s.append_pair("q", q); }
    if let Some(m) = last_month { s.append_pair("last_month", m); }
    format!("/artportfolio/htmx/posts?{}", s.finish())
}
```

`url` is already a direct dependency. Expected value, tested:

```rust
assert_eq!(
    load_more_url(1, Some("100%"), Some("2026-07")),
    "/artportfolio/htmx/posts?page=1&q=100%25&last_month=2026-07"
);
assert_eq!(load_more_url(1, None, None), "/artportfolio/htmx/posts?page=1");
```

- [ ] **Step 6: Rework the render path**

`render_page` becomes `render_grid`, returning only the HTML — `feed_page` now gets its
total from `count_posts` instead of from the page length:

```rust
async fn render_grid(state: &Arc<AppState>, page: i64, q: Option<&str>, last_month: Option<&str>) -> String {
    let mut posts = crate::db::get_posts_page(&state.pool, q, page).await;
    let has_more = posts.len() > 20;
    if has_more { posts.truncate(20); }
    let groups = group_by_month(posts, last_month);
    // The next page must know which month this one ended on, or it renders a
    // duplicate divider for a month already on screen.
    let next_last_month = groups.last().map(|g| g.label.clone());
    PostGridTemplate {
        has_more,
        load_more_url: load_more_url(page + 1, q, next_last_month.as_deref()),
        is_first_page: page == 0,
        q: q.unwrap_or_default().to_string(),
        groups,
    }
    .render()
    .unwrap()
}
```

`feed_page` gains `Query(pq): Query<PageQuery>` so `?q=` survives a reload, calls
`normalize_q`, renders page 0 with `last_month: None`, and builds the head from
`count_posts`.

- [ ] **Step 7: `HX-Push-Url` — spec correction #6**

> **The spec's HTMX contract is wrong here.** It says the search field carries
> `hx-push-url="true"`. HTMX pushes the *request* URL, so that would put
> `/artportfolio/htmx/posts?q=loomis` in the address bar — a URL that reloads as a bare
> HTML fragment with no shell, no styles and no nav. The spec's own stated intent
> ("a filtered feed is linkable and survives reload; `feed_page` reads the same `q` off
> the query string") requires the *page* URL.

`htmx_posts` returns the header itself, for page 0 only:

```rust
// Page 0 is a search or a fresh filter — the address bar should read
// /artportfolio?q=…, which is a real page. Load more (page >= 1) pushes
// nothing: it appends to what is already on screen.
```

Build the pushed URL with `url::form_urlencoded` exactly as Step 5 does: `/artportfolio`
when `q` is `None`, `/artportfolio?q=…` otherwise. Return
`([( "HX-Push-Url", value )], Html(html))` on page 0 and plain `Html(html)` otherwise —
an `axum::response::Response` built from either branch.

- [ ] **Step 8: `api_posts` gains the same filter**

`get_posts_page(&state.pool, normalize_q(q.q.as_deref()).as_deref(), page)`. The JSON API
and the HTML feed must not drift on what "matching" means.

- [ ] **Step 9: Rewrite `post_grid.html` for month sections**

```html
{% for group in groups %}
{% let first_group = loop.index0 == 0 %}
<section class="art-month">
  {% if group.show_divider %}
  <div class="art-month__divider">
    <span class="art-month__label">{{ group.label }} · {{ group.count }} drawings</span>
    <span class="art-month__rule"></span>
  </div>
  {% endif %}
  <div class="art-month__grid">
    {% for post in group.posts %}
    {% let is_first = is_first_page && first_group && loop.index0 == 0 %}
    {% include "partials/post_card.html" %}
    {% endfor %}
  </div>
</section>
{% endfor %}
```

`first_group` is bound **before** the inner loop on purpose: Askama's inner `loop`
shadows the outer one, so `loop.index0` inside refers to the post, not the group.

**`#load-more` stays a sibling of the sections, after `{% endfor %}`, as a direct child
of `#feed`.** `hx-swap="outerHTML"` replaces it in place, so if it sat inside the last
month's `.art-month__grid`, page N+1's whole `<section>` would be appended *into* a
`columns` block and get column-broken. That compiles green and looks broken.

The button's `hx-get` becomes `{{ load_more_url }}`; the `#load-more` / `outerHTML`
contract is otherwise unchanged.

- [ ] **Step 10: The filtered empty state**

`empty_state.html` carries a debt note saying the filtered variant arrives with search.
This is that slice. With `q` non-empty it reads `> no drawings match "loomis".`, and
with `q` empty it keeps `> no drawings yet.` Delete the stale comment. The guard in
`post_grid.html` stays `{% if groups.is_empty() && is_first_page %}` — page 3 of nothing
just means the end, and must render neither variant.

- [ ] **Step 11: Tests**

In `src/routes/feed.rs`'s test module, alongside the rewritten `head_label` test:

```rust
#[test]
fn test_group_by_month_splits_on_the_iso_prefix() {
    // created_at: 2026-08-03, 2026-08-01, 2026-07-20, 2026-06-30, 2026-06-02
    let groups = group_by_month(posts, None);
    assert_eq!(groups.len(), 3);
    assert_eq!(groups[0].label, "2026-08");
    assert_eq!(groups[0].count, 2);
    assert_eq!(groups[1].label, "2026-07");
    assert_eq!(groups[1].count, 1);
    assert_eq!(groups[2].label, "2026-06");
    assert_eq!(groups[2].count, 2);
    assert!(groups.iter().all(|g| g.show_divider));
}

#[test]
fn test_last_month_suppresses_only_a_matching_leading_divider() {
    // page 1 opens with more 2026-07 posts, then rolls into 2026-06
    let groups = group_by_month(posts, Some("2026-07"));
    assert!(!groups[0].show_divider, "2026-07 is already on screen from page 0");
    assert_eq!(groups[0].label, "2026-07", "the label survives suppression");
    assert!(groups[1].show_divider);
}

#[test]
fn test_last_month_that_does_not_match_suppresses_nothing() {
    let groups = group_by_month(posts, Some("2026-05"));
    assert!(groups[0].show_divider);
}

#[test]
fn test_group_by_month_of_nothing_is_empty() {
    assert!(group_by_month(vec![], None).is_empty());
}

#[test]
fn test_normalize_q_treats_blank_as_absent() {
    assert_eq!(normalize_q(Some("  ")), None);
    assert_eq!(normalize_q(Some("")), None);
    assert_eq!(normalize_q(None), None);
    assert_eq!(normalize_q(Some("  loomis ")).as_deref(), Some("loomis"));
}

#[tokio::test]
async fn test_htmx_page_0_pushes_the_page_url_not_the_fragment_url() {
    // GET /artportfolio/htmx/posts?q=loomis
    assert_eq!(headers.get("HX-Push-Url").unwrap(), "/artportfolio?q=loomis");
}

#[tokio::test]
async fn test_load_more_pushes_no_url() {
    // GET /artportfolio/htmx/posts?page=1
    assert!(headers.get("HX-Push-Url").is_none(), "appending must not rewrite the address bar");
}
```

Plus the two `load_more_url` assertions from Step 5. Use the existing `test_app()` helper
for the two integration cases.

- [ ] **Step 12: Commit**

```bash
git add src/models.rs src/routes/feed.rs templates/artportfolio/partials
git commit -m "feat(artportfolio): month grouping, caption search and the real head count"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

### Task 3: The filter rail

**Class:** A (compiler/lint-gated)

**Why this class:** Askama compiles templates into the binary, so a missing field or a
malformed tag is a build error. The rail's appearance is decided by the Task 5 browser
checkpoint, not by a reviewer.

**Files:**
- Create: `templates/artportfolio/partials/filter_rail.html`
- Modify: `templates/artportfolio/feed.html`
- Modify: `templates/partials/post_card.html` — one attribute
- Modify: `src/routes/feed.rs` — `FeedTemplate` gains `q: String`

**Interfaces:**
- Consumes: `normalize_q` and the `q` plumbing from Task 2.
- Produces: the ids and classes Tasks 4 and 5 target — `#art-search`, `.art-rail`,
  `.art-rail__legend`, `.art-main`, and `tabindex="-1"` on `article.hm-post`.

- [ ] **Step 1: `FeedTemplate` gains `q`**

`q: String`, empty when there is no search. `feed_page` already computes it in Task 2 —
this is the field that carries it into the search input's `value`.

- [ ] **Step 2: Write `filter_rail.html`**

Search field plus keyboard legend, and nothing else. Collections, tags and visibility
are slices 2 and 3; a "Filters" sheet holding one input would be an empty gesture.

Per the design brief: 34px field, `hm-input-wrap` + `hm-input` (both already in the
stylesheet from Plan A), a leading `search` glyph, a trailing `/` keycap, placeholder
`Search captions`. Legend in 11px mono above a hairline: `J / K — next, previous` and
`/ — search`.

HTMX contract on the input itself — HTMX includes a named input's own value, so no
`hx-include` is needed:

```html
<input class="hm-input" type="search" name="q" id="art-search" value="{{ q }}"
       placeholder="Search captions" autocomplete="off" spellcheck="false"
       hx-get="/artportfolio/htmx/posts"
       hx-trigger="keyup changed delay:200ms, search"
       hx-target="#feed" hx-swap="innerHTML">
```

No `hx-push-url` — Task 2's `HX-Push-Url` header does that job correctly. The `search`
trigger covers the native clear button in `type="search"`.

Wrap it in a `<form onsubmit="return false">` so Enter does not navigate away.

- [ ] **Step 3: Rework `feed.html`'s body**

`.art-body` becomes the flex row the brief describes — `aside` 236px, `main` flex 1,
32px gap:

```html
<div class="art-body">
  {% include "artportfolio/partials/filter_rail.html" %}
  <div class="art-main">
    <div id="feed">{{ initial_posts_html|safe }}</div>
  </div>
</div>
```

`#feed` stays the HTMX target and the inline-first-page optimisation is preserved.
`.art-feed` stays the outermost element in `{% block content %}` — it is the marker
`syncBodyTheme` keys off in both shells. The `{% if is_admin %}` composer keeps its
position, markup and `hx-swap="afterbegin"` contract.

- [ ] **Step 4: Make cards focusable**

Add `tabindex="-1"` to `<article class="hm-post">` in `templates/partials/post_card.html`.
`J`/`K` move focus, and an element with no tabindex cannot receive it. `-1` keeps the
cards out of the Tab order — a 20-card feed that swallows Tab would be worse than no
keyboard support at all.

This file has three callers; the attribute is harmless in all three.

- [ ] **Step 5: Commit**

```bash
git add templates/artportfolio src/routes/feed.rs templates/partials/post_card.html
git commit -m "feat(artportfolio): search rail and keyboard legend"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

### Task 4: CSS — per-month masonry, dividers, and the rail column

**Class:** A (compiler/lint-gated)

**Why this class:** `tests/static_assets.rs` decides the one failure mode a machine can
catch here (a nested comment marker dropping the next rule); the browser checkpoint
decides the rest. There is no logic.

**Files:**
- Modify: `static/style.css` — inside the existing `/* ── Drawing Portfolio ── */`
  section, around lines 1391–1540

**Interfaces:**
- Consumes: the class names from Tasks 2 and 3.
- Produces: nothing later tasks read.

- [ ] **Step 1: Move `columns` off `#feed` and onto the month grid**

This is the binding, non-obvious change. A single masonry with inline dividers does not
compose: CSS columns flow content down column 1 before column 2, so a "full-width"
divider would span one column and the chronology would read vertically per column. Each
month gets its own `columns` block instead.

```css
body.art-page .art-month__grid {
  columns: 3;
  column-gap: var(--space-6);
}
```

**`body.art-page #feed` keeps `display: block; gap: normal`.** Only `columns` and
`column-gap` move. Line 68 of this file sets `#feed { display: flex; flex-direction:
column; gap: … }` for the old feed; drop the override and the month sections become flex
items with a gap nobody designed — the same class of bug that cost Plan A a browser
checkpoint. A flex container also ignores `columns` silently.

`body.art-page #feed .hm-post { break-inside: avoid; margin-bottom: … }` is a descendant
selector and still matches through the new wrappers. Leave it.

`.load-more` and `.art-empty` no longer sit inside a column context — they are children
of `#feed`. Their `column-span: all` becomes inert but harmless; keep or drop with a
comment, do not leave it looking load-bearing.

- [ ] **Step 2: The month divider**

Per the brief: 11px mono, uppercase, tracking `.10em`, `--text-faint`, reading
`2026-07 · 5 drawings`, with a 1px rule filling the rest of the row. Flex row, label then
a `flex: 1` hairline at `--border-subtle`. Margin below it, and above it for every
section after the first.

- [ ] **Step 3: The rail column**

```css
body.art-page .art-body { display: flex; gap: var(--space-8); align-items: flex-start; }
body.art-page .art-rail { flex: 0 0 236px; position: sticky; top: 80px; }
body.art-page .art-main { flex: 1; min-width: 0; }
```

`min-width: 0` is not optional — a flex item defaults to `min-width: auto`, and a
multi-column child would push the row wider than the viewport instead of narrowing.

`top: 80px` clears the 56px sticky header with room to breathe, exactly as the brief
specifies.

- [ ] **Step 4: Responsive bands**

The bands stay disjoint, and the rail's collapse point (900px) is deliberately not the
masonry's (780px):

| Width | Rail | Masonry |
|---|---|---|
| ≥ 1180px | 236px, sticky at `top: 80px` | 3 columns |
| 900–1179px | 236px, sticky | 2 columns |
| 780–899px | full-width search above the grid | 2 columns |
| < 780px | full-width search above the grid | 1 column |

```css
@media (max-width: 1179px) { body.art-page .art-month__grid { columns: 2; } }
@media (max-width: 899px) {
  body.art-page .art-body { display: block; }
  body.art-page .art-rail { position: static; width: auto; margin-bottom: var(--space-7); }
  body.art-page .art-rail__legend { display: none; }
}
@media (max-width: 779px) { body.art-page .art-month__grid { columns: 1; } }
```

The legend hides below 900px because the keys it documents need a hardware keyboard.

Update the existing `@media (max-width: 1179px)` and `@media (max-width: 779px)` blocks
rather than adding rival ones — they currently target `#feed`.

- [ ] **Step 5: The search icon**

`.hm-icon--search` is referenced by the rail. If Plan A did not define its mask, add it
beside the existing icon rules, pointing at `/static/icons/search.svg` — the file is on
disk from Plan A Task 1, applied as a CSS mask over `currentColor` like its siblings.

- [ ] **Step 6: Check the comment guard directly**

```bash
cargo test --workspace test_static_css_has_no_nested_comment_markers -- --nocapture
```

Expected: PASS. A failure names the 1-based line of the first nested `/*`.

- [ ] **Step 7: Commit**

```bash
git add static/style.css
git commit -m "feat(artportfolio): per-month masonry, dividers and the rail column"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

### Task 5: `artfeed.js` — the keyboard layer

**Class:** A (compiler/lint-gated)

**Why this class:** `node --check` inside `verify.sh` decides the syntax — the check that
exists because a nested palette entry once broke `palette.js` and a reviewer missed it.
The behaviour is decided by the browser checkpoint below.

**Files:**
- Create: `static/artfeed.js`
- Modify: `templates/base.html` — one `<script>` tag
- Modify: `templates/admin.html` — the identical tag
- Modify: `CLAUDE.md` — the `/artportfolio` route description

**Interfaces:**
- Consumes: `#art-search` and `tabindex="-1"` on `article.hm-post` from Task 3.
- Produces: nothing.

- [ ] **Step 1: Write `artfeed.js`**

Follow `static/palette.js` exactly: a named init function guarded by an existence check,
called from **both** `DOMContentLoaded` and `htmx:afterSwap`. `hx-boost` replaces body
children without a reload, so `DOMContentLoaded` fires once per real page load only; the
guard is what keeps the second call a no-op rather than a duplicate listener.

Behaviour:

- `/` focuses and selects `#art-search`, and `preventDefault()`s so the slash does not
  land in the field.
- `Esc` blurs the search field.
- `J` / `K` step focus forward / backward through `#feed .hm-post`, with
  `focus({preventScroll: true})` then `scrollIntoView({block: 'nearest'})`.
- **All single-letter handling is suppressed when `document.activeElement` is an
  `input`, `textarea`, `select`, or a `contentEditable` element** — the design system's
  reserved-key rule. `Esc` is handled *before* that guard, since blurring the field is
  the one thing that must work while typing in it.
- Modifier chords are ignored (`ctrlKey`, `metaKey`, `altKey`) so `Ctrl+K` still reaches
  the palette.
- Cards are queried fresh on each keypress — HTMX replaces `#feed`'s contents on every
  search, so a cached NodeList would point at detached nodes.
- Reduced motion: `window.matchMedia('(prefers-reduced-motion: reduce)').matches` selects
  `behavior: 'auto'` instead of `'smooth'`.

- [ ] **Step 2: Load it from both shells**

`<script src="/static/artfeed.js" defer></script>` beside `palette.js` in
`templates/base.html` **and** `templates/admin.html`. It is inert on pages with no
`#art-search` and no `.hm-post`, and loading it in one shell only is exactly the drift
the both-shells rule exists to prevent.

- [ ] **Step 3: Update `CLAUDE.md`**

The `src/routes/feed.rs` bullet still describes `GET /artportfolio/htmx/posts?page=N`
and says nothing about `q`, `last_month`, month grouping or the search rail. One
paragraph, and the test count in the Tests section.

- [ ] **Step 4: Commit**

```bash
git add static/artfeed.js templates/base.html templates/admin.html CLAUDE.md
git commit -m "feat(artportfolio): slash-to-search and J/K card navigation"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

## Browser checkpoint (after Task 5)

Required by the user-global rule that UI changes are opened in a real browser before
being called done. Plan A's checkpoint found three defects the whole test suite could
not see; this one has a specific target list because the `columns` move is the same
class of change that caused the worst of them.

```bash
SQLX_OFFLINE=true cargo run       # :3000, no .env needed — everything defaults to localhost
```

**The dev DB has zero posts.** Seed rows with Python's `sqlite3` module (there is no
`sqlite3` CLI on this machine), setting `created_at` explicitly to span **three or more
months**, with at least one month straddling the 20-post page boundary so `last_month`
is actually exercised. **Delete the seed rows when done** and leave the table as found.

- [ ] `getComputedStyle(el).display === 'block'` and `columnCount === '3'` for **every**
      `.art-month__grid` at ≥1180px. This is the assertion Plan A learned the hard way:
      a flex container ignores `columns` in silence.
- [ ] Each month divider spans the **full content width**, not one column's width.
- [ ] Type `loomis` in the rail: the feed swaps, the head label reads
      `N drawings · matching "loomis"`, and the address bar reads `/artportfolio?q=loomis`
      — **not** `/artportfolio/htmx/posts?q=loomis`. Reload it; the filtered feed comes
      back with the query still in the field.
- [ ] Search for `100%`: results are only captions containing a literal `100%`, not
      every drawing.
- [ ] Clear the search: the full feed returns and the address bar goes back to
      `/artportfolio`.
- [ ] Load more across a month boundary: **no duplicate divider** for the month page 0
      ended on, and the appended section is not column-broken inside the previous month.
- [ ] `/` focuses the search field and no slash is typed into it; `Esc` blurs it.
- [ ] `J` and `K` walk the cards with a visible focus ring; neither fires while the
      search field has focus.
- [ ] `Ctrl+K` still opens the palette from `/artportfolio`.
- [ ] Log in and upload one image: it appears at the top of `#feed` as an `hm-post`.
      (The upload prepends to `#feed`, above the first month section — a known cosmetic
      wrinkle of slice 4's tray being deferred. Record what it actually looks like.)
- [ ] `/`, `/tasks` are visually unchanged and `<body class="">` on both.
- [ ] Navigate `/artportfolio → /tasks → /artportfolio` boosted: `art-page` attaches and
      detaches, and the keyboard layer still works after the second arrival — that is
      the `htmx:afterSwap` re-bind.

Environment caveats measured this session, do not fight them: `resize_window` reports
success but never changes `innerWidth`, so the 900px and 390px bands cannot be rendered
— verify those rules through the CSSOM and **say plainly that they were not rendered**.
Dark Reader repaints colours, so computed colours are not the site's; layout, geometry
and fonts are reliable.

---

## Final review

One review of the whole plan's diff, on the most capable model — per `plan-economics` §4,
Class A and B tasks get no per-task reviewer, and this plan has no Class C task.

Reviewer's first questions:

1. Does `body.art-page #feed` still set `display: block`, and does every
   `.art-month__grid` set `columns`?
2. Does `#load-more` sit outside the month sections?
3. Is the LIKE pattern escaped in the order `\`, `%`, `_`, and is `ESCAPE '\'` present in
   **both** the filtered SELECT and the filtered COUNT?
4. Does `git status .sqlx/` show delete + add rather than modify?

## Self-review against the spec

| Spec requirement | Task |
|---|---|
| `get_posts_page(pool, q, page)` renames `get_posts`, keeps the N+1 probe | 1 |
| `count_posts(pool, q)`, full page render only | 1, 2 §6 |
| LIKE escaping in Rust: `\`, `%`, `_`, then `%…%`, with `ESCAPE '\'` | 1 §1–3 |
| A search for `100%` does not match every row | 1 §5 |
| `PageQuery` gains `q` and `last_month` | 2 §Interfaces |
| Month grouping in the handler, keyed on `created_at[..7]` | 2 §3 |
| `MonthGroup { label, count, posts }` | 2 §1 |
| Month `<section>`s, each its own `columns` block, full-bleed divider | 2 §9, 4 §1–2 |
| `last_month` suppresses the duplicate divider | 2 §3, §11 |
| `filter_rail.html` — search + keyboard legend only | 3 §2 |
| `artfeed.js` — `/`, `Esc`, `J`, `K`, both shells, reduced motion | 5 |
| `api_posts` gains the same `q` filter | 2 §8 |
| Rail 236px sticky `top: 80px` ≥900px, collapsed below | 4 §3–4 |
| Page head states the real total; search variant | 2 §4 |
| Filtered empty state names the query | 2 §10 |
| Search is linkable and survives reload | 2 §7 (corrected) |

**Not in this plan** (slices 2–5): visibility model, collections, tags, multi-upload
tray, select mode, batch actions, the `?` overlay. The wordmark stays `Portfolio` — see
Plan A's "Open item: the wordmark"; it is a site-wide decision, not this section's.

## Spec corrections this plan makes

6. **`hx-push-url="true"` on the search field would push the fragment URL.** HTMX pushes
   the URL it requested, so the address bar would read
   `/artportfolio/htmx/posts?q=loomis` — which reloads as a bare fragment with no shell.
   The server returns `HX-Push-Url: /artportfolio?q=…` instead, on page 0 only, so Load
   more does not rewrite the address bar while appending. Task 2 Step 7.

(Corrections 1–5 are recorded in Plan A and are not repeated here.)
