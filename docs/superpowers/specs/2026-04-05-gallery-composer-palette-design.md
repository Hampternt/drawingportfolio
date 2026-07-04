# Design Spec: Gallery Composer, Optional Caption, Image Compression, Command Palette

**Date:** 2026-04-05
**Status:** Approved

---

## Context

The site currently requires uploading images exclusively through the `/admin` page, forces a non-empty caption on every post, stores raw image bytes with no compression, and has no keyboard-driven navigation. This spec covers four improvements:

1. A floating post composer embedded in the gallery page (no more admin-only uploads)
2. Optional caption (currently enforced non-empty at DB, app, and HTML levels)
3. Smart image compression at upload time, with user opt-out for large files
4. A command palette (Ctrl+K) for keyboard-driven site navigation and actions

---

## Feature 1 — Gallery Post Composer

### What
A **"+ New Post" button** appears in the top-right of the art feed, visible only when the user is logged in. Clicking it opens a **floating composer bubble** anchored near the button. The composer contains:

- A **Post Format selector** — pill buttons: "Single Image" (active), "Gallery" (greyed, stub), "Board" (greyed, stub)
- The **format body** — for Single Image: a file drop zone, an optional caption textarea, the compression opt-out checkbox (conditional), and an Upload button
- A close/dismiss control

The composer submits to the existing `POST /api/admin/posts` endpoint via HTMX multipart. On success, the new post card is prepended to the feed and the composer resets (stays open for quick successive uploads).

### Auth
The `+ New Post` button and composer HTML are rendered conditionally by Askama — only present in the DOM when `AuthSession` is valid. The upload endpoint already requires `AuthSession`, so there is no new auth surface.

### Extensibility
- The DB gets a `format TEXT NOT NULL DEFAULT 'single'` column added via migration
- A `PostFormat` enum (`Single`) is added in Rust — new variants added here later
- The JS composer reads a `data-format` attribute to decide which field set to render — adding a new format later = new data-format handler, no structural change
- The greyed-out Gallery/Board pills are rendered as disabled stubs now, activated later by adding their handlers

### Files affected
- `src/routes/feed.rs` — pass `is_admin: bool` to the feed template
- `src/routes/admin.rs` — read optional `format` field from multipart (default `"single"`)
- `src/db.rs` — `insert_post` gains a `format` parameter; migration adds the column
- `src/models.rs` — `Post` struct gains `format: String`
- `templates/artportfolio/feed.html` — composer HTML + New Post button (admin-gated)
- `static/style.css` — composer bubble styles
- `migrations/` — new migration for `format` and `file_size_bytes` columns

---

## Feature 2 — Optional Caption

### What
Caption becomes fully optional end-to-end. An empty or absent caption is valid.

### Changes
- **No DB migration needed for caption** — the column stays `TEXT NOT NULL`. SQLite cannot modify existing column constraints, but none is needed: the app layer will always pass `""` when the user provides no caption. Existing rows are unaffected.
- **`src/db.rs`**: `insert_post` accepts `caption: &str` (empty string allowed)
- **`src/routes/admin.rs`**: remove the non-empty caption check; if `caption` field is absent from the multipart, default to `""`
- **`post_card_html()`**: only render `<p class="caption">` if caption is non-empty
- **`admin_post_card_html()`**: same conditional render
- **`templates/admin.html`**: remove `required` from the caption textarea (for the legacy admin page)
- **Composer HTML**: caption textarea has no `required` attribute

---

## Feature 3 — Image Compression at Upload

### What
When the server receives an image upload, if the file exceeds **4 MB**, it is automatically re-encoded as WebP at 85% quality before being stored in the bucket. The user can opt out by checking a **"Keep original file without compression"** checkbox in the composer — unchecked (default) means compress.

The checkbox only appears in the composer UI when the selected file exceeds 4 MB (detected client-side via a JS `change` event on the file input).

### Server behaviour
- `MAX_COMPRESS_BYTES`: constant set to `4 * 1024 * 1024`
- Multipart field `keep_original`: `"true"` if checkbox is checked, absent/`"false"` otherwise
- If image bytes exceed `MAX_COMPRESS_BYTES` AND `keep_original != "true"`: decode with the `image` crate, re-encode as WebP (quality 85), store the WebP bytes
- If under threshold or opt-out: store original bytes as-is
- The stored extension reflects the actual format (`.webp` for compressed output)
- `file_size_bytes` (the final stored size) is saved to the DB regardless

### DB
New column: `file_size_bytes INTEGER NOT NULL DEFAULT 0` — added in the same migration as `format`.

### Cargo.toml
Add `image = { version = "0.25", default-features = false, features = ["webp", "jpeg", "png"] }`

### Files affected
- `Cargo.toml`
- `src/routes/admin.rs` — compression logic, read `keep_original` field
- `src/db.rs` — `insert_post` gains `file_size_bytes: i64`
- `src/models.rs` — `Post` struct gains `file_size_bytes: i64`
- `templates/artportfolio/feed.html` — JS file-size check + conditional checkbox render
- `static/style.css` — checkbox style

---

## Feature 4 — Command Palette

### What
Pressing **Ctrl+K** (or Cmd+K on Mac) anywhere on the site opens a floating search palette. Typing filters commands by label/keywords. Arrow keys navigate, Enter executes, Escape dismisses. A backdrop click also dismisses.

### Command registry
A `COMMANDS` array defined in `base.html` (or a linked `static/palette.js`). Each entry:
```js
{ label: String, keywords: [String], action: Function, adminOnly?: Boolean }
```

Initial commands:
| Label | Keywords | Action | Admin only |
|---|---|---|---|
| Upload new drawing | upload, post, new, image | Opens gallery composer | Yes |
| Go to Art Portfolio | feed, gallery, art, drawings | `location.href = '/artportfolio'` | No |
| Go to Hub | home, hub, index | `location.href = '/'` | No |
| Admin panel | admin, settings | `location.href = '/admin'` | Yes |

### Auth gating
Askama renders `<script>const IS_ADMIN = true/false;</script>` in `base.html` based on session state. The palette filters out `adminOnly` commands when `IS_ADMIN` is false. Visitors never see upload or admin commands.

The feed route (and any other public route extending `base.html`) currently has no auth awareness. A new **`OptionalAuth` extractor** is needed in `src/middleware.rs` — it checks the session cookie and returns `bool` without ever redirecting. Routes that need to pass `is_admin` to their template use this extractor. The existing `AuthSession` extractor (redirect-on-fail) is unchanged and still used by all admin API routes.

### Extensibility
Adding a new command = adding one object to `COMMANDS`. The palette engine (render, filter, keyboard nav) is written once and never touched again.

### Files affected
- `src/middleware.rs` — new `OptionalAuth` extractor (returns `bool`, never redirects)
- `templates/base.html` — `IS_ADMIN` script tag, Ctrl+K listener, palette overlay HTML, `COMMANDS` array (or `<script src="/static/palette.js">`)
- `static/palette.js` — palette engine (alternatively inlined)
- `static/style.css` — palette overlay styles
- `src/routes/feed.rs`, `src/routes/hub.rs` — gain `OptionalAuth` extractor, pass `is_admin` to template

---

## Data model summary (post-migration)

```sql
CREATE TABLE posts (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    caption         TEXT NOT NULL,          -- unchanged; app sends "" when empty
    image_url       TEXT NOT NULL,
    format          TEXT NOT NULL DEFAULT 'single',       -- new
    file_size_bytes INTEGER NOT NULL DEFAULT 0,           -- new
    created_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
```

Migration adds `format` and `file_size_bytes` columns only. Caption column is not altered.

---

## Verification

1. **Optional caption**: upload via composer with no caption — post appears in feed with no caption element rendered
2. **Compression**: upload a PNG > 4MB with checkbox unchecked — confirm stored URL ends in `.webp`, file in bucket is smaller; repeat with checkbox checked — original format stored
3. **Composer auth gate**: log out, visit `/artportfolio` — no `+ New Post` button in DOM; log in — button appears, composer opens and uploads successfully
4. **Format column**: after upload, query `SELECT format FROM posts ORDER BY id DESC LIMIT 1` — returns `'single'`
5. **Command palette**: press Ctrl+K on any page — palette opens; type "upload" — Upload command appears (logged in) or is absent (logged out); press Enter — composer opens
6. **Existing admin page**: verify legacy `/admin` upload still works with empty caption
7. **cargo test**: all existing tests pass (db, feed, admin route tests)
