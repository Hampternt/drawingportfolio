# /artportfolio visual layer — Plan A (of 2) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILLS: `plan-economics` (this repo's task
> classes, review policy and plan sizing) then superpowers:executing-plans to
> execute task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Render `/artportfolio` in the Hampter Design System, with real image
dimensions stored so the masonry does not reflow on image load.

**Architecture:** Every painting rule is scoped to `body.art-page`, so no other
section changes a pixel. Token declarations land on bare `:root` — they are inert
custom properties and affect nothing that does not read them. Card rendering moves
out of the `post_card_html()` format string in `src/routes/feed.rs` and into a real
Askama template, so the inline first page, the HTMX pagination route and the upload
response all render one source of truth.

**Slice:** When this plan is done, `/artportfolio` is fully restyled and
deployable: dark design-system page head, `hm-post` cards in a 3-column masonry,
Load more, empty state, and a restyled admin composer that still uploads. Plan B
picks up caption search (`get_posts_page(q)` + `count_posts`), month grouping and
dividers, the filter rail, and the `/` `J` `K` keyboard layer.

Source spec: `docs/superpowers/specs/2026-08-09-artportfolio-visual-layer-design.md`.
Design authority: `docs/design/artportfolio-redesign/README.md` — where it and the
`.dc.html` prototypes disagree, the README wins.

## Global Constraints

- **Blast radius.** After this plan, `/`, `/tasks`, `/fitness`, `/admin` and
  `/drinks` render byte-identically to before. Every rule that paints something is
  written under `body.art-page`.
- **`tokens/legacy-nocturne.css` is excluded entirely.** It repoints every `--noc-*`
  variable and would shift `/fitness` from `#9184d9` to `#B48EF7` as a side effect.
  Migrating the fitness tracker is a separate future slice.
- **`tokens/base.css` is not lifted as-is.** It sets unscoped
  `body { background: var(--surface-page) }` plus global `a`, `p`, `h1`–`h4` and
  `:focus-visible` rules. Its resets are rewritten under `body.art-page`.
- **No nested `/* */` in `static/style.css`.** CSS comments do not nest; a `/*`
  inside a running comment silently drops the next rule. `tests/static_assets.rs`
  fails the build on it. The five lifted token files contain 43 comment markers
  between them — strip every comment from lifted blocks.
- **No `<style>` blocks in templates that extend `base.html`** (CLAUDE.md). All CSS
  goes in `static/style.css` under one named section comment.
- **Both shells.** Any global feature (script, style, header element) lands in
  **both** `templates/base.html` and `templates/admin.html`. `admin.html` is
  standalone by recorded exception.
- **`object-fit` is never set on post media.** The design system's
  `components/components.css:201` sets `object-fit: cover`, which crops. Drawings
  render uncropped: `width: 100%; height: auto; display: block`.
- **`position INTEGER` is not implemented.** The handoff's column would be written
  by upload order and read by nothing — the design only ever sorts newest-first.
- **Copy is fixed.** Intro line, verbatim:
  `Mostly practice — studies, drills, and the odd finished piece.`
  Wordmark: `hampter.` with the full stop in `--violet-400`, scoped to `.art-page`.
- **Motion.** 130ms controls, 190ms surfaces, `cubic-bezier(.2,.8,.3,1)`. Press is
  `translateY(1px)`, never a scale. `@media (prefers-reduced-motion: reduce)` zeroes
  every duration token.
- **Focus.** One ring everywhere: 2px page colour, then 2px violet, via
  `:focus-visible` scoped to `body.art-page`.
- **Chrono is pinned to `0.4.34`** — do not bump it; `0.4.35+` renames `Duration`.

**Verification for every task:** `./scripts/verify.sh` — all green, output quoted in
the report. Never accept bare `cargo test`: the root `Cargo.toml` is both a package
and the workspace root, so `cargo test` runs 52 of 229 tests and silently skips
`drinkinggame`.

**`cargo sqlx prepare` runs once, in Task 2**, because Task 2 is the only schema
change and every later task must compile against the regenerated cache.

**Browser checkpoint:** after Task 5 only. Not per task.

**Baseline:** `878d7d5`, `./scripts/verify.sh` green, 229 tests.

---

### Task 1: Self-hosted fonts and icons

**Class:** A (compiler/lint-gated)

**Why this class:** The acceptance is `./scripts/verify.sh` plus a byte-count check
on ten files. Nothing here has behaviour a reviewer could reason about.

**Files:**
- Create: `static/fonts/` — 7 `.woff2`
- Create: `static/icons/` — 3 `.svg`

**Interfaces:**
- Consumes: nothing.
- Produces: the paths Task 3's `@font-face` and `.hm-icon` mask rules reference:
  `/static/fonts/archivo-700.woff2`, `/static/fonts/archivo-800.woff2`,
  `/static/fonts/archivo-900.woff2`, `/static/fonts/space-grotesk-400.woff2`,
  `/static/fonts/space-grotesk-500.woff2`, `/static/fonts/ibm-plex-mono-400.woff2`,
  `/static/fonts/ibm-plex-mono-500.woff2`, `/static/icons/search.svg`,
  `/static/icons/chevron-down.svg`, `/static/icons/x.svg`.

- [ ] **Step 1: Copy the five faces already in the repo**

`drinkinggame/assets/fonts/` holds Archivo 500/600/700/800/900 and Space Grotesk
400/500/600/700, byte-identical to the design system's copies. Copy only the five
this page uses:

```bash
mkdir -p static/fonts static/icons
cp drinkinggame/assets/fonts/archivo-700.woff2       static/fonts/
cp drinkinggame/assets/fonts/archivo-800.woff2       static/fonts/
cp drinkinggame/assets/fonts/archivo-900.woff2       static/fonts/
cp drinkinggame/assets/fonts/space-grotesk-400.woff2 static/fonts/
cp drinkinggame/assets/fonts/space-grotesk-500.woff2 static/fonts/
```

Copy, do not symlink or reference across crates: `drinkinggame`'s fonts are
`include_bytes!`-compiled into the binary and never served from disk, while
`static/` is rsynced by `deploy.yml:65`. The two have different delivery paths.

- [ ] **Step 2: Fetch the two IBM Plex Mono faces**

Not in the repo — the `_ds` bundle ships Archivo and Space Grotesk only. Take the
**latin** subset URL from each weight (the last `@font-face` block Google returns,
the one whose `unicode-range` starts `U+0000-00FF`):

```bash
UA='Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 Chrome/120 Safari/537.36'
for w in 400 500; do
  url=$(curl -sS -A "$UA" \
    "https://fonts.googleapis.com/css2?family=IBM+Plex+Mono:wght@$w&display=swap" \
    | grep -B4 'U+0000-00FF' | grep -o 'https://[^)]*\.woff2')
  curl -sS -o "static/fonts/ibm-plex-mono-$w.woff2" "$url"
done
```

The `-A` user-agent matters: without a modern UA, Google Fonts serves TTF rather
than woff2.

- [ ] **Step 3: Fetch the three Lucide icons**

Pinned to v0.462.0, the version named in the handoff. Only three — the visitor view
needs no others; later slices add theirs.

```bash
for i in search chevron-down x; do
  curl -sS -o "static/icons/$i.svg" \
    "https://unpkg.com/lucide-static@0.462.0/icons/$i.svg"
done
```

- [ ] **Step 4: Verify all ten files are real**

```bash
ls -l static/fonts static/icons
file static/fonts/*.woff2 | grep -c "Web Open Font Format"   # expect 7
grep -lc "<svg" static/icons/*.svg                            # expect all three
```

A `curl` that 404s writes a short HTML error page, not a font. Any `.woff2` that
`file` does not identify as "Web Open Font Format", or any `.svg` with no `<svg`
element, is a failed fetch — re-run before proceeding.

Do not test the SVGs with `head -c 40 … == "<svg"`: lucide-static prepends
`<!-- @license lucide-static v0.462.0 - ISC -->` to every icon, so that check reads
as a failure on a perfectly good file. The license line is itself useful — it is how
you confirm the pinned version actually came back.

- [ ] **Step 5: Commit**

```bash
git add static/fonts static/icons
git commit -m "feat(artportfolio): self-host the design system's fonts and icons"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

### Task 2: Migration 012 — image dimensions

**Class:** C (logic tests cannot encode — reviewer required)

**Why this class:** Not the `ALTER`s, which are tame and whose idempotence is
machine-checkable. The risk is the sqlx offline cache: six of the ~60 `.sqlx`
entries describe rows from `posts`, and adding two columns changes the row shape
every one of them records. A partial regeneration compiles locally and fails in an
unrelated query later, which is the confusing mode the spec warns about. A reviewer
asking "did you regenerate the whole cache, and does `git status` show all six
entries rewritten?" is what earns this class.

**Files:**
- Create: `migrations/012_image_dimensions.sql`
- Modify: `src/db.rs:82` (`get_posts`), `src/db.rs:93` (`insert_post`), the
  `run_migrations()` tail, and the `insert_post` calls in `db.rs`'s own tests
- Modify: `src/models.rs:4-13` (`Post`)
- Modify: `src/routes/admin.rs` (upload handler — pass real dimensions)
- Modify: `src/routes/feed.rs:217` (`insert_post` call in tests)
- Modify: `.sqlx/` (regenerated wholesale)

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `Post { …, pub image_width: i64, pub image_height: i64 }` — appended **after**
    `file_size_bytes` and **before** `created_at` is not required; append at the end
    of the struct, and list them last in both SELECT column lists so struct field
    order and column order agree.
  - `db::insert_post(pool, caption, image_url, webp_url, avif_url, format,
    file_size_bytes, image_width: i64, image_height: i64) -> Post`

- [ ] **Step 1: Write the migration**

`migrations/012_image_dimensions.sql`:

```sql
ALTER TABLE posts ADD COLUMN image_width  INTEGER NOT NULL DEFAULT 0;
ALTER TABLE posts ADD COLUMN image_height INTEGER NOT NULL DEFAULT 0;
```

Existing rows keep `0` and simply render without dimension attributes. No backfill —
the originals are in object storage and re-decoding them is not worth a startup
stall.

- [ ] **Step 2: Register it in `run_migrations()`**

Append after the migration 011 block in `src/db.rs`, following the identical
`let _ =` shape the other eleven use. The `let _ =` is what makes re-running
idempotent: SQLite has no `ADD COLUMN IF NOT EXISTS`, so a second run returns a
duplicate-column error that is deliberately discarded.

```rust
// Migration 012: intrinsic image dimensions (masonry needs them to avoid reflow)
let _ = sqlx::query(include_str!("../migrations/012_image_dimensions.sql"))
    .execute(pool)
    .await;
```

Note the numbering diverges from the handoff, which reserved 012 for visibility.
Because the visual layer ships first, **012 is image dimensions** and visibility
becomes 013 in slice 2. Migration numbers follow ship order.

- [ ] **Step 3: Add the two fields to `Post`**

`src/models.rs`, appended to the struct. `Post` derives `Serialize` and is returned
by `GET /artportfolio/api/posts`, so both fields become part of that endpoint's
public JSON. This is an accepted additive change — the API is unversioned and
single-consumer.

- [ ] **Step 4: Update both SELECT column lists in `src/db.rs`**

`get_posts` and the tail of `insert_post` both use `sqlx::query_as!(Post, "SELECT
id, caption, … FROM posts …")` with an **explicit** column list. The macro does not
infer new fields — omit them and the build fails. Add `image_width, image_height` to
the end of both lists.

- [ ] **Step 5: Widen `insert_post`**

Two new parameters, `image_width: i64, image_height: i64`, appended to the
signature; add both to the `INSERT` column list and one `?` each to `VALUES`.

**There are 7 `insert_post(` call sites** across `src/db.rs`, `src/routes/feed.rs`
and `src/routes/admin.rs`. The compiler enumerates them all — pass `0, 0` at every
test call site, and real values only at the one in `admin.rs` (Step 6).

- [ ] **Step 6: Read dimensions from the image header in the upload handler**

> **Spec correction — the spec's premise here is false.** It says "the upload
> handler already decodes the image with the `image` crate to generate the WebP
> variant, so `.dimensions()` is free at that point." It is not. The server does not
> generate a WebP variant at all: `src/routes/admin.rs:169` is
> `let webp_url = image_url.clone(); // already WebP from client` — the browser
> converts via canvas before upload. The *only* use of the `image` crate anywhere in
> `src/` is `admin.rs:16`, inside `encode_as_avif`, which runs in a detached
> `tokio::spawn` **after** the response is sent and whose `dimensions()` never
> returns to the handler. There is no decoded image in scope at `insert_post`.
>
> CLAUDE.md carries the same stale claim ("On upload, WebP and AVIF variants are
> generated concurrently via `tokio::join!`"). Logged under Known debts.

Do **not** add a full decode to the request path to fix this — `image::load_from_memory`
on a 35 MB upload costs seconds of latency on every post, to obtain two integers.

Read the header only. `image` is pinned at `0.25.10`; `ImageReader::into_dimensions()`
parses just enough of the header to return `(u32, u32)` and never touches pixel data:

```rust
use std::io::Cursor;

let (image_width, image_height) = image::ImageReader::new(Cursor::new(&bytes))
    .with_guessed_format()
    .ok()
    .and_then(|r| r.into_dimensions().ok())
    .map(|(w, h)| (w as i64, h as i64))
    .unwrap_or((0, 0));
```

Placed **before** the `state.storage.upload(...)` call that consumes `bytes`.

The `unwrap_or((0, 0))` is the degradation path and is deliberate: a header the
crate cannot parse gives exactly today's behaviour — a card with no `width`/`height`
attributes — rather than a failed upload. Dimensions are a layout optimisation, not
data worth rejecting a drawing over.

Note `Cargo.toml` builds `image` with `default-features = false, features =
["webp", "jpeg", "png"]`, which matches the three types
`validate_magic_bytes` accepts. A format outside that set cannot reach this line.

Nothing else in this file changes: `admin_post_card_html()` and the three-layer
35 MB limit (nginx `client_max_body_size`, Axum `DefaultBodyLimit`,
`MAX_IMAGE_BYTES`) are untouched.

- [ ] **Step 7: Write the tests**

In `src/db.rs`'s test module. Three cases, with expected values:

```rust
#[tokio::test]
async fn test_migrations_are_idempotent() {
    let pool = SqlitePoolOptions::new().connect("sqlite::memory:").await.unwrap();
    run_migrations(&pool).await;
    run_migrations(&pool).await; // must not panic — duplicate columns are discarded
    let posts = get_posts(&pool, 0).await;
    assert!(posts.is_empty());
}

#[tokio::test]
async fn test_insert_post_persists_dimensions() {
    let pool = SqlitePoolOptions::new().connect("sqlite::memory:").await.unwrap();
    run_migrations(&pool).await;
    let post = insert_post(&pool, "c", "u", "", "", "single", 0, 1600, 900).await;
    assert_eq!(post.image_width, 1600);
    assert_eq!(post.image_height, 900);

    let fetched = &get_posts(&pool, 0).await[0];
    assert_eq!(fetched.image_width, 1600, "dimensions survive the round trip");
    assert_eq!(fetched.image_height, 900);
}

#[tokio::test]
async fn test_legacy_rows_default_to_zero_dimensions() {
    let pool = SqlitePoolOptions::new().connect("sqlite::memory:").await.unwrap();
    run_migrations(&pool).await;
    // Simulate a pre-012 row: insert without touching the new columns.
    sqlx::query("INSERT INTO posts (caption, image_url, webp_url, avif_url, format, file_size_bytes) VALUES ('old', 'u', '', '', 'single', 0)")
        .execute(&pool).await.unwrap();
    let post = &get_posts(&pool, 0).await[0];
    assert_eq!(post.image_width, 0, "legacy rows read back as 0, not NULL");
    assert_eq!(post.image_height, 0);
}
```

The third test is the one that matters: `NOT NULL DEFAULT 0` on an `ALTER` over an
existing table is exactly where SQLite's behaviour is worth pinning down, because a
`NULL` here would make `Post`'s `i64` fail to decode at runtime rather than at
compile time.

- [ ] **Step 8: Run the sqlx offline-cache ritual**

Required by CLAUDE.md for any schema change in the `drawingportfolio` crate (unlike
`drinkinggame`, which is runtime-checked and has no cache).

```bash
DATABASE_URL=sqlite:portfolio.db cargo sqlx prepare
git status --short .sqlx/
```

> **The dev DB must be fully migrated first, and a copied one may not be.** The
> `portfolio.db` in this worktree was copied from the main checkout and was dated
> months back — it predated migrations 007–011 entirely, so `prepare` failed with
> `no such table: drawing_tasks` on 37 unrelated queries. There is no `sqlite3` CLI
> on this machine, and the binary cannot migrate the DB itself because `prepare`
> runs `cargo check`, which needs the columns to already exist. Apply
> `migrations/*.sql` in order with Python's bundled `sqlite3` module, tolerating
> duplicate-column errors exactly as `run_migrations()`'s `let _ =` does. Confirm
> `PRAGMA table_info(posts)` lists `image_width` and `image_height`, and that all
> twelve tables exist, before re-running `prepare`.

**Expected `.sqlx/` diff: 3 deleted, 3 added — not 6 modified.**

> **Spec correction.** The spec says "six of the 60 `.sqlx` entries reference
> `posts`; adding two columns changes the row shape every one of them describes."
> It does not. A cache entry records *the columns its query selects*, not the
> table's shape, so only the three queries whose SQL text actually changed are
> affected — `get_posts`'s SELECT, `insert_post`'s INSERT and `insert_post`'s
> SELECT. The other three (`SELECT image_url, webp_url, avif_url FROM posts …`,
> `DELETE FROM posts …`, `UPDATE posts SET avif_url …`) select explicit subsets and
> are correctly untouched.
>
> And they appear as **delete + add, never modify**: `.sqlx` filenames are
> content-addressed by a hash of the query string, so changing a query's text
> renames its cache file. Looking for ` M` in `git status` would read a correct
> regeneration as a failure.

Verify the three new entries actually carry the columns:

```bash
grep -l "image_width" .sqlx/*.json | wc -l   # expect 3
```

An empty `.sqlx/` diff after a column change means the command did not run against
the migrated DB.

- [ ] **Step 9: Commit**

```bash
git add migrations/012_image_dimensions.sql src/models.rs src/db.rs \
        src/routes/admin.rs src/routes/feed.rs .sqlx
git commit -m "feat(artportfolio): store intrinsic image dimensions (migration 012)"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

### Task 3: The `Drawing Portfolio` CSS section

**Class:** A (compiler/lint-gated)

**Why this class:** `tests/static_assets.rs` decides the one failure mode that
matters here (nested comment markers dropping a rule), and the browser checkpoint
after Task 5 decides the rest. There is no logic for a reviewer to reason about.

**Files:**
- Modify: `static/style.css` — one new section appended at the end

**Interfaces:**
- Consumes: Task 1's font and icon paths.
- Produces: the class names Task 5's templates emit — `art-feed`, `art-head`,
  `hm-grid-bg`, `hm-post`, `hm-post__media`, `hm-post__body`, `hm-post__caption`,
  `hm-post__meta`, `hm-btn`, `hm-btn--secondary`, `hm-btn--ghost`, `hm-btn--sm`,
  `hm-btn--md`, `hm-iconbtn`, `hm-icon`, `hm-kbd`, `hm-kbd-group`, `hm-card`,
  `hm-input`, `hm-field`, `art-empty`.

- [ ] **Step 1: Open the section**

Append to `static/style.css`, following the existing named-section convention:

```css
/* ── Drawing Portfolio ─────────────────────────────────────────── */
```

Everything below lands under this one heading, in the order given by Steps 2–8.

- [ ] **Step 2: Lift the five token files verbatim onto `:root`**

Source of truth, in this order:

```
docs/design/artportfolio-redesign/_ds/hampter-design-system-03c25988-bba8-4fc5-801f-653a333b24c3/tokens/colors.css
…/tokens/typography.css
…/tokens/spacing.css
…/tokens/shape.css
…/tokens/motion.css
```

They contain only `:root` custom-property declarations — inert, and they affect
nothing that does not read them, which is why they may sit unscoped.

**Strip every comment while lifting.** The five files carry 43 `/*` markers between
them (colors 23, motion 14, shape 3, typography 2, spacing 1); pasted inside this
section they are the exact shape `tests/static_assets.rs` fails on. Values must be
copied exactly — the handoff's instruction is to recreate the type and colour
"exactly; do not approximate".

Spot-check after pasting, against the handoff's colour table
(`docs/design/artportfolio-redesign/README.md`, "Design tokens"):
`--ink-950: #0B0910` · `--ink-900: #0E0C14` · `--ink-850: #17141F` ·
`--ink-700: #262232` · `--ink-050: #F2EEF8` · `--violet-400: #B48EF7` ·
`--violet-300: #D6BDFB`. Body text `#CDC6DD`, muted `#8D87A0`, faint `#5F5876`.

**Do not lift `tokens/base.css`** (Step 4 rewrites it) **or
`tokens/legacy-nocturne.css`** (excluded — see Global Constraints).

- [ ] **Step 3: Declare the seven faces**

Seven `@font-face` blocks pointing at Task 1's files. Every one gets
`font-display: swap` and an explicit `font-weight`, matching how the drinking game
declares its own faces.

| family | weights | file |
|---|---|---|
| `Archivo` | 700, 800, 900 | `/static/fonts/archivo-<w>.woff2` |
| `Space Grotesk` | 400, 500 | `/static/fonts/space-grotesk-<w>.woff2` |
| `IBM Plex Mono` | 400, 500 | `/static/fonts/ibm-plex-mono-<w>.woff2` |

- [ ] **Step 4: Rewrite `base.css`'s resets under `body.art-page`**

Read `…/tokens/base.css` and port its rules, prefixing each selector with
`body.art-page`. Its unscoped `body { background: var(--surface-page) }` becomes
`body.art-page { background: var(--ink-950); color: #CDC6DD; }`, and its global `a`,
`p`, `h1`–`h4` and `:focus-visible` rules each gain the same prefix. Linking the
file as-is would turn every page on the site dark on the next request — that is the
whole reason this step exists rather than an `@import`.

Focus ring, scoped: `body.art-page :focus-visible` → 2px `--ink-950` then 2px
`--violet-400`, via `box-shadow: 0 0 0 2px <page>, 0 0 0 4px <violet>` with
`outline: none`.

- [ ] **Step 5: Lift the component subset**

From `…/components/components.css`, only these: `hm-icon`, `hm-btn` (`--secondary`,
`--ghost`, `--sm`, `--md`), `hm-iconbtn`, `hm-kbd` + `hm-kbd-group`, `hm-card`,
`hm-post*`, `hm-input*`, `hm-field*`, `hm-grid-bg`. Each selector gains a
`body.art-page` prefix.

`Tag` and `Badge` are **not** lifted — they belong to slices 3 and 2 and would be
dead rules here.

Two deviations from that file, both binding:

1. **`components.css:201` sets `.hm-post__media img { object-fit: cover }`. Drop
   it.** Replace with `width: 100%; height: auto; display: block;`. The README
   mandates drawings render uncropped at natural ratio and the design system's own
   imagery rule says drawings are "never force-cropped to a tile".
2. `.hm-post__media` keeps `background: var(--ink-900)` plus the blueprint grid, so
   the well is visible while the image loads.

- [ ] **Step 6: Page layout, masonry and the blueprint grid**

- `.art-feed` — content column max 1180px, 24px gutters, 32px page padding at
  ≥1280px.
- `.art-head` — 48px top / 32px bottom padding, hairline
  `rgba(242,238,248,.07)` bottom border, background `--ink-950` plus the blueprint
  grid.
- Blueprint grid, used by `.hm-grid-bg`, `.art-head` and `.hm-post__media` — a 32px
  repeating linear-gradient pair at 4.5% white:
  `rgba(242,238,248,.045)`.
- Masonry — `columns: 3; column-gap: 16px;` on the grid container, with
  `.hm-post { break-inside: avoid; margin-bottom: 16px; }`.

**Plan A's grid is one flat block.** Month `<section>`s and dividers are Plan B; do
not build them here — the `Vec<MonthGroup>` they need does not exist yet.

- [ ] **Step 7: Restyle the admin composer**

The `{% if is_admin %}` composer in `feed.html` survives this slice unchanged in
markup (see Task 5) — the multi-upload tray that replaces it is slice 4. Without
this step it renders as unstyled light-mode controls against a near-black page.

Add a small `body.art-page` block covering `#composer-wrap`, `#new-post-btn`,
`#composer`, `.composer-formats`, `.fmt-btn`, `.drop-zone`, `#composer textarea`,
`.composer-actions`, `.btn-primary`, `.btn-secondary`: card fill `--ink-850`, 1px
`rgba(242,238,248,.10)`, radius 8px, Space Grotesk 400 15px, violet primary button.
Reuse the `hm-btn` and `hm-input` values rather than inventing new ones.

Do not touch the composer's markup or its inline `<script>` — those are slice 4's.

- [ ] **Step 8: Responsive, motion and reduced motion**

Breakpoints are disjoint by design; the handoff's "2 columns 780–1179" and "under
900px the rail collapses" overlap, so the 780–899 band is spelled out. The rail
itself is Plan B — implement the masonry column counts now and leave the rail
column to Plan B:

| Width | Masonry |
|---|---|
| ≥ 1180px | 3 columns |
| 900–1179px | 2 columns |
| 780–899px | 2 columns |
| < 780px | 1 column |

Motion: 130ms controls, 190ms surfaces, `cubic-bezier(.2,.8,.3,1)`. Card hover
lifts `translateY(-2px)`, border to `rgba(242,238,248,.20)`, adds `--shadow-2`.
Press is `translateY(1px)`, never a scale.

```css
@media (prefers-reduced-motion: reduce) {
  body.art-page { --motion-fast: 0ms; --motion-mid: 0ms; --motion-slow: 0ms; }
  body.art-page * { transition-duration: 0ms !important; animation-duration: 0ms !important; }
}
```

Use the real token names from `tokens/motion.css` as lifted in Step 2, not the
placeholder names above.

- [ ] **Step 9: Check the comment guard directly**

```bash
cargo test --workspace test_static_css_has_no_nested_comment_markers -- --nocapture
```

Expected: PASS. A failure names the 1-based line of the first nested `/*` — that is
a lifted comment that Step 2 or Step 5 should have stripped.

- [ ] **Step 10: Commit**

```bash
git add static/style.css
git commit -m "feat(artportfolio): design-system tokens and components under body.art-page"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

### Task 4: Generalise the body-class sync in both shells

**Class:** A (compiler/lint-gated)

**Why this class:** `node --check` in `scripts/verify.sh` decides the syntax; the
Task 5 browser checkpoint decides the behaviour. The logic is a two-line
`classList.toggle` with no state.

**Files:**
- Modify: `templates/base.html:18-31`
- Modify: `templates/admin.html` — the identical inline script block

**Interfaces:**
- Consumes: nothing.
- Produces: the marker contract Task 5's `feed.html` must satisfy — the outermost
  element inside `{% block content %}` is `<div class="art-feed">`, and
  `syncBodyTheme` keys off `main .art-feed` and nothing else.

- [ ] **Step 1: Generalise the script in `base.html`**

`hx-boost="true"` on `<body>` means HTMX replaces body *children*, not body
*attributes* — so `{% block body_class %}` is lost on every boosted navigation. The
existing script already solves this for `fitness-dark`; it now derives both classes
from a marker element in the incoming content:

```js
(function () {
  function syncBodyTheme() {
    document.body.classList.toggle('fitness-dark',
      !!document.querySelector('main .fitness-page'));
    document.body.classList.toggle('art-page',
      !!document.querySelector('main .art-feed'));
  }
  document.addEventListener('htmx:afterSwap', function (e) {
    if (e.target === document.body) syncBodyTheme();
  });
  document.addEventListener('htmx:historyRestore', syncBodyTheme);
})();
```

Keep the `e.target === document.body` guard. Without it every HTMX fragment swap —
including Load more, which swaps `#load-more` — re-runs the query against a
partially-updated DOM.

Update the comment above it to say *theme classes* rather than *the fitness theme*.

- [ ] **Step 2: Apply the identical edit to `admin.html`**

Required by the CLAUDE.md rule that `admin.html` is standalone and receives every
global feature `base.html` gets. The two blocks must stay byte-identical — a
`diff <(grep -A14 syncBodyTheme templates/base.html) <(grep -A14 syncBodyTheme templates/admin.html)`
should be empty.

- [ ] **Step 3: Commit**

```bash
git add templates/base.html templates/admin.html
git commit -m "feat(artportfolio): derive art-page alongside fitness-dark on boosted nav"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

### Task 5: Card, grid and page templates

**Class:** A (compiler/lint-gated)

**Why this class:** Askama compiles templates into the binary, so a malformed
template, a missing field or a renamed variable is a build error, not a runtime
surprise. The render assertions in Step 6 cover the escaping and the conditional
attributes.

**Files:**
- Overwrite: `templates/partials/post_card.html` (currently 234 bytes, zero
  references, and stale — it predates `<picture>`/AVIF)
- Create: `templates/artportfolio/partials/post_grid.html`
- Create: `templates/artportfolio/partials/empty_state.html`
- Modify: `templates/artportfolio/feed.html`
- Modify: `src/routes/feed.rs` — delete `post_card_html()` and
  `render_posts_html()`, add the template structs, port four tests
- Modify: `src/routes/admin.rs:210-214` — render the new card template

**Interfaces:**
- Consumes: `Post { …, image_width: i64, image_height: i64 }` from Task 2; the class
  names from Task 3; the `main .art-feed` marker contract from Task 4.
- Produces:
  ```rust
  #[derive(Template)]
  #[template(path = "partials/post_card.html")]
  pub struct PostCardTemplate<'a> { pub post: &'a Post, pub is_first: bool }

  #[derive(Template)]
  #[template(path = "artportfolio/partials/post_grid.html")]
  struct PostGridTemplate<'a> { posts: &'a [Post], has_more: bool, next_page: i64, is_first_page: bool }
  ```
  `PostCardTemplate` is `pub` because `src/routes/admin.rs` renders it.

- [ ] **Step 1: Write the card template**

Overwrite `templates/partials/post_card.html`. Askama auto-escapes every `{{ }}`, so
the manual `html_escape()` calls disappear — that is the point of the move.

Requirements, all of which the current format string already satisfies and which
must survive:

- Root `<article class="hm-post" id="post-{{ post.id }}">`.
- `<picture>` with an AVIF `<source>` emitted only when `post.avif_url` is
  non-empty, then a WebP `<source>` under the same condition, then the `<img>`
  falling back to `post.image_url`. The empty-URL-means-variant-failed behaviour is
  load-bearing: AVIF is encoded in a `tokio::spawn` after the response is sent, so a
  freshly-uploaded post genuinely has an empty `avif_url` for a second or two.
- `{% if is_first %}loading="eager" fetchpriority="high"{% else %}loading="lazy"{% endif %}`.
- `width`/`height` attributes emitted **only when both dimensions are non-zero**:
  `{% if post.image_width > 0 && post.image_height > 0 %}`. A `width="0"` would
  collapse the image.
- Caption in `<p class="hm-post__caption">`, omitted entirely when
  `post.caption.is_empty()`.
- `<div class="hm-post__meta">` carrying `post.created_at`.

- [ ] **Step 2: Write the grid and empty-state templates**

`post_grid.html` — **flat**, no month sections. It renders each post through the
card partial with `is_first` true only for index 0 of the first page, then the Load
more control when `has_more`:

```html
<button class="hm-btn hm-btn--secondary hm-btn--sm"
        hx-get="/artportfolio/htmx/posts?page={{ next_page }}"
        hx-target="#load-more"
        hx-swap="outerHTML">
  Load more <span class="hm-icon hm-icon--chevron-down"></span>
</button>
```

wrapped in `<div class="load-more" id="load-more">`. The `#load-more` /
`outerHTML` contract is unchanged from today — do not alter it, Plan B extends the
URL with `q` and `last_month` and relies on this shape.

Month grouping and dividers are Plan B. `post_grid.html` takes `posts: &[Post]`.

`empty_state.html` — one mono line behind a `>` prompt, per the handoff:
`> no drawings yet.` in `<div class="art-empty">`. Rendered only when the post list
is empty on the first page.

- [ ] **Step 3: Rewrite `feed.html`**

- `{% block body_class %}art-page{% endblock %}`.
- `{% block head %}` gains two preloads — `archivo-800.woff2` and
  `space-grotesk-400.woff2`, `rel="preload" as="font" type="font/woff2" crossorigin`.
  In `feed.html`, **not** `base.html`: putting them in the shell would make every
  other page pay for fonts it does not use.
- Outermost element inside `{% block content %}` is `<div class="art-feed">` —
  this is the marker Task 4's `syncBodyTheme` keys off. Nothing else satisfies it.
- Page head inside it, per the spec:

  | element | type | copy |
  |---|---|---|
  | micro-label | 11px IBM Plex Mono, uppercase, tracking .10em, `--text-faint` | `{{ post_count }} drawings · newest first` |
  | `h1` | Archivo 800 40px/1.22, −0.025em, `--text-strong` | `Drawing Portfolio` |
  | intro | Space Grotesk 400 15px/1.5, `--text-muted`, max 68ch | `Mostly practice — studies, drills, and the odd finished piece.` |

  Plan A has no `count_posts` — that is Plan B. Render the micro-label from
  `posts.len()` of the first page for now and let Plan B replace it with the real
  total. Say so in a template comment so it is not mistaken for finished.

- **Keep the entire `{% if is_admin %}` composer block, its inline `<script>`, and
  its `hx-target="#feed" hx-swap="afterbegin"` exactly as they are today.** The
  multi-upload tray that replaces it is slice 4; deleting it now would leave admins
  with no way to upload from this page. Task 3 Step 7 styles it.
- `<div id="feed">{{ initial_posts_html|safe }}</div>` stays — the inline first-page
  optimisation is preserved, so no second round trip on load.

- [ ] **Step 4: Rewrite the two render paths in `src/routes/feed.rs`**

Delete `post_card_html()` and `render_posts_html()`. Both `feed_page` and
`htmx_posts` render `PostGridTemplate` instead, which keeps the two paths producing
identical markup — the same no-drift property the shared helper provided.

**Keep `html_escape()`** — it is `pub` and `src/routes/admin.rs`'s
`admin_post_card_html()` still uses it for the admin dashboard.

`api_posts` is unchanged in this plan; it gains the `q` filter in Plan B.

- [ ] **Step 5: Fix the third caller in `src/routes/admin.rs`**

`src/routes/admin.rs:210-214` is the upload response, selected by
`source == "gallery"` — the hidden `<input name="source" value="gallery">` in the
composer form. It calls `crate::routes::feed::post_card_html(&post, false)`.

The spec says `post_card_html` has "both callers"; it has **three**. Point this one
at `PostCardTemplate { post: &post, is_first: false }.render()`. Miss it and a fresh
upload injects a legacy `.post-card` into a feed of `hm-post` cards, which looks
broken until the page is reloaded — and the build still passes, because the function
would only be deleted if all three callers moved.

The `else` branch (`admin_post_card_html(&post)`, for `source != "gallery"`) is
untouched — that is the `/admin` dashboard's card and it keeps its `.post-card`
markup, which is why Task 3 leaves the existing `.post-card` rules in place.

- [ ] **Step 6: Port the four card tests**

`src/routes/feed.rs`'s tests at ~236, ~255, ~276 and ~301 all call
`post_card_html()`, which no longer exists. All four are **ported**, not deleted —
they assert behaviour the new template must keep. Each becomes
`PostCardTemplate { post: &post, is_first: false }.render().unwrap()` and keeps its
existing assertions. Every `Post` literal in them gains `image_width: 0,
image_height: 0`.

Add two cases:

```rust
#[test]
fn test_post_card_emits_dimensions_when_known() {
    // post with image_width: 1600, image_height: 900
    let html = PostCardTemplate { post: &post, is_first: false }.render().unwrap();
    assert!(html.contains("width=\"1600\""));
    assert!(html.contains("height=\"900\""));
}

#[test]
fn test_post_card_omits_dimensions_when_zero() {
    // post with image_width: 0, image_height: 0
    let html = PostCardTemplate { post: &post, is_first: false }.render().unwrap();
    assert!(!html.contains("width=\"0\""), "a zero width would collapse the image");
    assert!(!html.contains("height=\"0\""));
}
```

The escaping test is the one to watch: it asserts `&lt;script&gt;` is present and
`<script>` is not. Askama's auto-escaping must produce the same result the
hand-rolled `html_escape` did — if it escapes differently (e.g. `&#x27;` for
apostrophes), the assertion still holds, but check the actual output once rather
than assuming.

- [ ] **Step 7: Commit**

```bash
git add templates/partials/post_card.html templates/artportfolio \
        src/routes/feed.rs src/routes/admin.rs
git commit -m "feat(artportfolio): render cards from Askama templates in design-system markup"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

## Browser checkpoint (after Task 5)

Required by the user-global rule that UI changes are opened in a real browser before
being called done. Type-checks verify code correctness, not feature correctness.

```bash
cargo run       # :3000
```

- [ ] `/artportfolio` at **1280px** — 3 columns, dark page, blueprint grid visible in
      the page head and in any media well still loading.
- [ ] **900px** — 2 columns. **390px** — 1 column.
- [ ] A card with a caption and one without both render correctly; the first card
      has `loading="eager"`, the rest `loading="lazy"` (check in devtools).
- [ ] **Load more** appends without a second divider or a layout jump.
- [ ] Log in and upload one image through the composer: it must appear at the top of
      the feed styled as an `hm-post`, **not** as a legacy `.post-card`. This is the
      Task 5 Step 5 path and it is the one a passing build cannot catch.
- [ ] Navigate `/artportfolio → /tasks → /artportfolio` via the nav (boosted): the
      `art-page` class attaches and detaches, and `/tasks` never renders dark.
- [ ] `/`, `/tasks`, `/fitness`, `/admin` are visually unchanged. `/fitness` accent
      is still `#9184d9`, **not** `#B48EF7` — if it shifted, `legacy-nocturne.css`
      leaked in.
- [ ] Devtools → *Emulate CSS `prefers-reduced-motion: reduce`* → card hover has no
      transition.

Automation caveat (recorded in project memory): a browser-automation tab stays
backgrounded, which freezes animations and makes screenshots unreliable. Measure
with `getComputedStyle` / `getAnimations` rather than screenshots, or use human
eyes.

---

## Final review

One review of the whole plan's diff, on the most capable model — per
`plan-economics` §4, Class A and B tasks get no per-task reviewer, and Task 2 (Class
C) gets one at the time it lands.

Reviewer's first question for Task 2: *does the migration commit show exactly three
`.sqlx` entries deleted and three added, and do all three new ones contain
`image_width`?* Not "six modified" — see the correction in Task 2 Step 8.

---

## Self-review against the spec

| Spec requirement | Task |
|---|---|
| Design-system visual layer scoped to `body.art-page` | 3 |
| Tokens lifted; `base.css` rewritten; `legacy-nocturne.css` excluded | 3 §2, §4 |
| `object-fit: cover` dropped | 3 §5 |
| Seven woff2 self-hosted, two preloaded | 1, 5 §3 |
| Three Lucide icons self-hosted | 1 |
| Migration 012 image dimensions, no backfill | 2 |
| `Post` gains `image_width`/`image_height`; API JSON additive | 2 §3 |
| Dimensions written on upload from the already-decoded image | 2 §6 |
| sqlx offline-cache ritual, whole-cache regen | 2 §8 |
| `post_card_html()` / `render_posts_html()` deleted; `html_escape()` kept | 5 §4 |
| `templates/partials/post_card.html` overwritten, rendered by both paths | 5 §1, §4, §5 |
| `width`/`height` only when non-zero | 5 §1, §6 |
| `loading`/`fetchpriority` treatment preserved | 5 §1 |
| `<picture>` + empty-variant fallback preserved | 5 §1 |
| Inline first-page optimisation preserved | 5 §3 |
| `art-page` derived on boosted nav, in **both** shells | 4 |
| Responsive column counts, disjoint bands | 3 §8 |
| Motion, press, focus ring, reduced motion | 3 §4, §8 |
| Nested-comment guard | 3 §9 |
| Browser check at 1280/900/390 + other sections unchanged | checkpoint |

**Deferred to Plan B, deliberately:** `get_posts_page(pool, q, page)`,
`count_posts(pool, q)`, LIKE escaping, `PageQuery { q, last_month }`, month grouping
and `MonthGroup`, month `<section>` dividers, `filter_rail.html`, `artfeed.js`
(`/` `J` `K`), the real page-head count, and `api_posts`'s `q` filter.

**Not in either plan** (slices 2–5): visibility model, collections, tags,
multi-upload tray, select mode, batch actions, `?` overlay.

## Two spec corrections this plan makes

Both found by reading the code, both binding:

1. **`post_card_html()` has three callers, not two.** The spec says "both callers
   render Askama templates instead" and names only the two in `feed.rs`.
   `src/routes/admin.rs:211` is the third — the upload response. Task 5 Step 5.
2. **The admin composer is not in the spec's `feed.html` description.** The spec
   describes `feed.html` as "Shell: extends `base.html`, sets `body_class`, page
   head, rail + main flex, `#feed`" with no mention of the existing
   `{% if is_admin %}` composer block. Since the multi-upload tray that replaces it
   is slice 4, deleting it here would strip upload from the page for two slices. It
   survives unchanged in markup and gets scoped CSS. Task 3 Step 7, Task 5 Step 3.
