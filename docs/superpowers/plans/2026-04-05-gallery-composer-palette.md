# Gallery Composer, Optional Caption, Compression, Command Palette — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a floating gallery post composer, make caption optional, compress large images at upload, and add a Ctrl+K command palette.

**Architecture:** Rust/Axum backend with Askama SSR templates and HTMX for dynamic updates. New DB columns (`format`, `file_size_bytes`) are added via idempotent `ALTER TABLE` in `run_migrations()`. An `OptionalAuth` extractor (non-redirecting) threads `is_admin: bool` into public page templates so the composer and palette commands appear only when logged in. Image compression happens server-side using the `image` crate before S3 upload, gated by a multipart `keep_original` field. The command palette is pure JS (`static/palette.js`) that builds its overlay using DOM methods (no innerHTML on dynamic content).

**Tech Stack:** Rust · Axum 0.8 · SQLite/sqlx 0.8 · Askama 0.15 · HTMX · `image` 0.25 · Vanilla JS

---

## File Map

| File | Change |
|---|---|
| `migrations/002_add_post_fields.sql` | **Create** — `format` + `file_size_bytes` columns |
| `src/db.rs` | Modify — `run_migrations`, `insert_post`, `get_posts` |
| `src/models.rs` | Modify — add `format`, `file_size_bytes` to `Post` |
| `src/middleware.rs` | Modify — add `OptionalAuth` extractor |
| `src/routes/feed.rs` | Modify — `FeedTemplate` context, `feed_page` handler, `post_card_html` |
| `src/routes/hub.rs` | Modify — `HubTemplate` context, `hub_page` handler |
| `src/routes/admin.rs` | Modify — optional caption, compression, `source` field, `insert_post` call |
| `Cargo.toml` | Modify — add `image` crate |
| `templates/base.html` | Modify — IS_ADMIN script tag, palette.js script tag |
| `templates/artportfolio/feed.html` | Modify — composer bubble + New Post button |
| `templates/admin.html` | Modify — remove caption `required`, add IS_ADMIN + palette.js |
| `static/palette.js` | **Create** — command palette engine |
| `static/style.css` | Modify — composer + palette styles |

---

## Task 1: DB migration and model update

**Files:**
- Create: `migrations/002_add_post_fields.sql`
- Modify: `src/db.rs`
- Modify: `src/models.rs`

- [ ] **Step 1: Write the failing tests**

Add to the `#[cfg(test)]` block in `src/db.rs`, after `test_delete_nonexistent_post_returns_none`:

```rust
#[tokio::test]
async fn test_insert_post_stores_format_and_filesize() {
    let pool = test_pool().await;
    let fmt = crate::models::PostFormat::Single.as_str();
    let post = insert_post(&pool, "hello", "https://example.com/img.jpg", fmt, 12345).await;
    assert_eq!(post.format, "single");
    assert_eq!(post.file_size_bytes, 12345);
}

#[tokio::test]
async fn test_insert_post_empty_caption() {
    let pool = test_pool().await;
    let fmt = crate::models::PostFormat::Single.as_str();
    let post = insert_post(&pool, "", "https://example.com/img.jpg", fmt, 0).await;
    assert_eq!(post.caption, "");
}
```

- [ ] **Step 2: Run to confirm they fail**

```bash
cargo test test_insert_post_stores_format_and_filesize test_insert_post_empty_caption 2>&1 | tail -20
```

Expected: compile error — `insert_post` doesn't accept 5 arguments yet.

- [ ] **Step 3: Create the migration file**

Create `migrations/002_add_post_fields.sql`:

```sql
ALTER TABLE posts ADD COLUMN format TEXT NOT NULL DEFAULT 'single';
ALTER TABLE posts ADD COLUMN file_size_bytes INTEGER NOT NULL DEFAULT 0;
```

- [ ] **Step 4: Update `run_migrations` in `src/db.rs`**

Replace:
```rust
pub async fn run_migrations(pool: &DbPool) {
    sqlx::query(include_str!("../migrations/001_initial.sql"))
        .execute(pool)
        .await
        .expect("failed to run migrations");
}
```

With:
```rust
pub async fn run_migrations(pool: &DbPool) {
    sqlx::query(include_str!("../migrations/001_initial.sql"))
        .execute(pool)
        .await
        .expect("failed to run migrations");

    // Migration 002: idempotent — errors on duplicate column are intentionally ignored
    let _ = sqlx::query("ALTER TABLE posts ADD COLUMN format TEXT NOT NULL DEFAULT 'single'")
        .execute(pool)
        .await;
    let _ = sqlx::query("ALTER TABLE posts ADD COLUMN file_size_bytes INTEGER NOT NULL DEFAULT 0")
        .execute(pool)
        .await;
}
```

- [ ] **Step 5: Update `Post` struct in `src/models.rs`**

Replace:
```rust
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Post {
    pub id: i64,
    pub caption: String,
    pub image_url: String,
    pub created_at: String,
}
```

With:
```rust
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Post {
    pub id: i64,
    pub caption: String,
    pub image_url: String,
    pub format: String,
    pub file_size_bytes: i64,
    pub created_at: String,
}
```

- [ ] **Step 5b: Add `PostFormat` enum to `src/models.rs`**

Add after the `Post` struct:
```rust
/// Extensibility hook: add new variants here as post formats are implemented.
#[derive(Debug, Clone, PartialEq)]
pub enum PostFormat {
    Single,
}

impl PostFormat {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Single => "single",
        }
    }
}

impl Default for PostFormat {
    fn default() -> Self {
        Self::Single
    }
}
```

This enum is the authoritative source for valid format strings. Use `PostFormat::Single.as_str()` wherever the literal `"single"` would appear in Rust code.

- [ ] **Step 6: Update `insert_post` and `get_posts` in `src/db.rs`**

Replace `insert_post`:
```rust
pub async fn insert_post(pool: &DbPool, caption: &str, image_url: &str, format: &str, file_size_bytes: i64) -> Post {
    let id = sqlx::query!(
        "INSERT INTO posts (caption, image_url, format, file_size_bytes) VALUES (?, ?, ?, ?) RETURNING id",
        caption, image_url, format, file_size_bytes
    )
    .fetch_one(pool)
    .await
    .expect("failed to insert post")
    .id;

    sqlx::query_as!(Post,
        "SELECT id, caption, image_url, format, file_size_bytes, created_at FROM posts WHERE id = ?", id
    )
    .fetch_one(pool)
    .await
    .expect("failed to fetch inserted post")
}
```

Replace `get_posts`:
```rust
pub async fn get_posts(pool: &DbPool, page: i64) -> Vec<Post> {
    let offset = page * 20;
    sqlx::query_as!(Post,
        "SELECT id, caption, image_url, format, file_size_bytes, created_at FROM posts ORDER BY created_at DESC LIMIT 21 OFFSET ?",
        offset
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default()
}
```

- [ ] **Step 7: Fix all existing call sites**

In `src/db.rs` tests — update every `insert_post` call (3 occurrences):
```rust
// Before:
insert_post(&pool, "test caption", "https://example.com/img.jpg").await
// After:
insert_post(&pool, "test caption", "https://example.com/img.jpg", crate::models::PostFormat::Single.as_str(), 0).await
```

In `src/routes/feed.rs` tests — fix the loop in `test_api_posts_has_more`:
```rust
for i in 0..21 {
    crate::db::insert_post(&pool, &format!("caption {i}"), "https://example.com/img.jpg", crate::models::PostFormat::Single.as_str(), 0).await;
}
```

Also update the `Post` struct literal in `test_post_card_html_escapes_content`:
```rust
let post = crate::models::Post {
    id: 1,
    caption: "<script>alert(1)</script>".to_string(),
    image_url: "https://example.com/img.jpg".to_string(),
    format: crate::models::PostFormat::Single.as_str().to_string(),
    file_size_bytes: 0,
    created_at: "2024-01-01T00:00:00".to_string(),
};
```

- [ ] **Step 8: Run all tests**

```bash
cargo test 2>&1 | tail -30
```

Expected: all tests pass. If sqlx complains about missing offline data, run:
```bash
cargo sqlx prepare --database-url sqlite:./portfolio.db
```
(requires `portfolio.db` to exist — run `cargo run` first to create it).

- [ ] **Step 9: Commit**

```bash
git add migrations/002_add_post_fields.sql src/db.rs src/models.rs src/routes/feed.rs
git commit -m "feat: add format and file_size_bytes columns to posts"
```

---

## Task 2: OptionalAuth extractor and IS_ADMIN template context

**Files:**
- Modify: `src/middleware.rs`
- Modify: `src/routes/feed.rs`
- Modify: `src/routes/hub.rs`
- Modify: `templates/base.html`

- [ ] **Step 1: Add `OptionalAuth` extractor to `src/middleware.rs`**

Add after the closing `}` of the `AuthSession` impl block (before `extract_session_cookie`):

```rust
/// Extractor: checks session without ever redirecting. Returns true if logged in.
pub struct OptionalAuth(pub bool);

impl FromRequestParts<Arc<AppState>> for OptionalAuth {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, state: &Arc<AppState>) -> Result<Self, Self::Rejection> {
        let is_admin = if let Some(id) = extract_session_cookie(parts) {
            db::get_session(&state.pool, &id).await.is_some()
        } else {
            false
        };
        Ok(OptionalAuth(is_admin))
    }
}
```

- [ ] **Step 2: Update `FeedTemplate` and `feed_page` in `src/routes/feed.rs`**

Replace:
```rust
#[derive(Template)]
#[template(path = "artportfolio/feed.html")]
struct FeedTemplate;

// ...

async fn feed_page() -> impl IntoResponse {
    Html(FeedTemplate.render().unwrap())
}
```

With:
```rust
#[derive(Template)]
#[template(path = "artportfolio/feed.html")]
struct FeedTemplate {
    is_admin: bool,
}

// ...

async fn feed_page(
    OptionalAuth(is_admin): crate::middleware::OptionalAuth,
) -> impl IntoResponse {
    Html(FeedTemplate { is_admin }.render().unwrap())
}
```

- [ ] **Step 3: Update `HubTemplate` and `hub_page` in `src/routes/hub.rs`**

Replace the entire file:
```rust
use axum::{Router, routing::get, response::{Html, IntoResponse}};
use askama::Template;
use std::sync::Arc;
use crate::AppState;

#[derive(Template)]
#[template(path = "hub/hub.html")]
struct HubTemplate {
    is_admin: bool,
}

async fn hub_page(
    OptionalAuth(is_admin): crate::middleware::OptionalAuth,
) -> impl IntoResponse {
    Html(HubTemplate { is_admin }.render().unwrap())
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(hub_page))
}
```

- [ ] **Step 4: Add IS_ADMIN script tag to `templates/base.html`**

Replace:
```html
  <script src="https://unpkg.com/htmx.org@2.0.4" integrity="sha384-HGfztofotfshcF7+8n44JQL2oJmowVChPTg48S+jvZoztPfvwD79OC/LTtG6dMp+" crossorigin="anonymous"></script>
  {% block head %}{% endblock %}
```

With:
```html
  <script src="https://unpkg.com/htmx.org@2.0.4" integrity="sha384-HGfztofotfshcF7+8n44JQL2oJmowVChPTg48S+jvZoztPfvwD79OC/LTtG6dMp+" crossorigin="anonymous"></script>
  <script>const IS_ADMIN = {% if is_admin %}true{% else %}false{% endif %};</script>
  {% block head %}{% endblock %}
```

- [ ] **Step 5: Build to confirm templates compile**

```bash
cargo build 2>&1 | grep "^error"
```

Expected: no errors.

- [ ] **Step 6: Commit**

```bash
git add src/middleware.rs src/routes/feed.rs src/routes/hub.rs templates/base.html
git commit -m "feat: add OptionalAuth extractor and IS_ADMIN template context"
```

---

## Task 3: Optional caption

**Files:**
- Modify: `src/routes/admin.rs`
- Modify: `src/routes/feed.rs`
- Modify: `templates/admin.html`

- [ ] **Step 1: Write a failing test for empty-caption card rendering**

Add to `src/routes/feed.rs` tests:
```rust
#[test]
fn test_post_card_empty_caption_omits_p_tag() {
    let post = crate::models::Post {
        id: 2,
        caption: "".to_string(),
        image_url: "https://example.com/img.jpg".to_string(),
        format: "single".to_string(),
        file_size_bytes: 0,
        created_at: "2024-01-01T00:00:00".to_string(),
    };
    let html = post_card_html(&post);
    assert!(!html.contains("class=\"caption\""), "empty caption must not render p.caption");
}
```

- [ ] **Step 2: Run to confirm it fails**

```bash
cargo test test_post_card_empty_caption_omits_p_tag 2>&1 | tail -10
```

Expected: FAIL.

- [ ] **Step 3: Update `post_card_html` in `src/routes/feed.rs`**

Replace:
```rust
pub fn post_card_html(post: &Post) -> String {
    format!(
        r#"<article class="post-card" id="post-{}">
  <img src="{}" alt="{}" loading="lazy">
  <p class="caption">{}</p>
  <small class="date">{}</small>
</article>"#,
        post.id,
        html_escape(&post.image_url),
        html_escape(&post.caption),
        html_escape(&post.caption),
        html_escape(&post.created_at),
    )
}
```

With:
```rust
pub fn post_card_html(post: &Post) -> String {
    let caption_html = if post.caption.is_empty() {
        String::new()
    } else {
        format!("  <p class=\"caption\">{}</p>\n", html_escape(&post.caption))
    };
    format!(
        r#"<article class="post-card" id="post-{}">
  <img src="{}" alt="{}" loading="lazy">
{caption_html}  <small class="date">{}</small>
</article>"#,
        post.id,
        html_escape(&post.image_url),
        html_escape(&post.caption),
        html_escape(&post.created_at),
    )
}
```

- [ ] **Step 4: Update `admin_post_card_html` in `src/routes/admin.rs`**

Replace:
```rust
fn admin_post_card_html(post: &crate::models::Post) -> String {
    format!(
        r##"<div class="admin-post" id="admin-post-{}">
  <img src="{}" alt="">
  <div class="info">
    <p>{}</p>
    <small>{}</small>
  </div>
  <button class="delete-btn"
          hx-delete="/api/admin/posts/{}"
          hx-target="#admin-post-{}"
          hx-swap="outerHTML"
          hx-confirm="Delete this post?">
    Delete
  </button>
</div>"##,
        post.id,
        html_escape(&post.image_url),
        html_escape(&post.caption),
        html_escape(&post.created_at),
        post.id,
        post.id,
    )
}
```

With:
```rust
fn admin_post_card_html(post: &crate::models::Post) -> String {
    let caption_html = if post.caption.is_empty() {
        String::new()
    } else {
        format!("    <p>{}</p>\n", html_escape(&post.caption))
    };
    format!(
        r##"<div class="admin-post" id="admin-post-{}">
  <img src="{}" alt="">
  <div class="info">
{caption_html}    <small>{}</small>
  </div>
  <button class="delete-btn"
          hx-delete="/api/admin/posts/{}"
          hx-target="#admin-post-{}"
          hx-swap="outerHTML"
          hx-confirm="Delete this post?">
    Delete
  </button>
</div>"##,
        post.id,
        html_escape(&post.image_url),
        html_escape(&post.created_at),
        post.id,
        post.id,
    )
}
```

- [ ] **Step 5: Remove non-empty caption validation in `src/routes/admin.rs`**

Replace lines 81–84 (the match destructure):
```rust
    let (caption, (bytes, content_type)) = match (caption, image_data) {
        (Some(c), Some(d)) if !c.trim().is_empty() => (c, d),
        _ => return (StatusCode::BAD_REQUEST, Html("Missing caption or image".to_string())).into_response(),
    };
```

With:
```rust
    let caption = caption.unwrap_or_default();
    let (bytes, content_type) = match image_data {
        Some(d) => d,
        None => return (StatusCode::BAD_REQUEST, Html("Missing image".to_string())).into_response(),
    };
```

- [ ] **Step 6: Remove `required` from caption textarea in `templates/admin.html`**

Replace:
```html
          <textarea name="caption" required placeholder="Write something..."></textarea>
```

With:
```html
          <textarea name="caption" placeholder="Caption (optional)"></textarea>
```

- [ ] **Step 7: Run all tests**

```bash
cargo test 2>&1 | tail -20
```

Expected: all tests pass.

- [ ] **Step 8: Commit**

```bash
git add src/routes/admin.rs src/routes/feed.rs templates/admin.html
git commit -m "feat: make caption optional throughout"
```

---

## Task 4: Image compression at upload

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/routes/admin.rs`

- [ ] **Step 1: Add `image` crate to `Cargo.toml`**

Add after the `mime = "0.3"` line:
```toml
image = { version = "0.25", default-features = false, features = ["webp", "jpeg", "png"] }
```

- [ ] **Step 2: Write a unit test for the compression function**

Add to `src/routes/admin.rs` tests (inside `#[cfg(test)]`):
```rust
#[test]
fn test_compress_to_webp_returns_webp_bytes() {
    // Minimal valid 1x1 red PNG
    let png: Vec<u8> = vec![
        0x89,0x50,0x4E,0x47,0x0D,0x0A,0x1A,0x0A,
        0x00,0x00,0x00,0x0D,0x49,0x48,0x44,0x52,
        0x00,0x00,0x00,0x01,0x00,0x00,0x00,0x01,
        0x08,0x02,0x00,0x00,0x00,0x90,0x77,0x53,0xDE,
        0x00,0x00,0x00,0x0C,0x49,0x44,0x41,0x54,
        0x08,0xD7,0x63,0xF8,0xCF,0xC0,0x00,0x00,
        0x00,0x02,0x00,0x01,0xE2,0x21,0xBC,0x33,
        0x00,0x00,0x00,0x00,0x49,0x45,0x4E,0x44,
        0xAE,0x42,0x60,0x82,
    ];
    let result = compress_to_webp(&png);
    assert!(result.is_ok(), "compression should succeed: {:?}", result.err());
    assert_eq!(&result.unwrap()[0..4], b"RIFF", "output should be a WebP (RIFF) file");
}
```

- [ ] **Step 3: Run to confirm it fails (function doesn't exist yet)**

```bash
cargo test test_compress_to_webp_returns_webp_bytes 2>&1 | tail -10
```

Expected: compile error.

- [ ] **Step 4: Add compression constant and function to `src/routes/admin.rs`**

Add after `const MAX_IMAGE_BYTES: usize = 35 * 1024 * 1024;`:
```rust
const MAX_COMPRESS_BYTES: usize = 4 * 1024 * 1024; // 4 MB

fn compress_to_webp(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let img = image::load_from_memory(bytes)
        .map_err(|e| format!("decode failed: {e}"))?;
    let mut buf = std::io::Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::WebP)
        .map_err(|e| format!("encode failed: {e}"))?;
    Ok(buf.into_inner())
}
```

- [ ] **Step 5: Run the test to confirm it passes**

```bash
cargo test test_compress_to_webp_returns_webp_bytes 2>&1 | tail -10
```

Expected: PASS.

- [ ] **Step 6: Update the multipart loop in `upload_post` to read all new fields**

Replace the variable declarations and the `while let` loop with:

```rust
    let mut caption = None::<String>;
    let mut image_data = None::<(Vec<u8>, String)>;
    let mut keep_original = false;
    let mut format = crate::models::PostFormat::Single.as_str().to_string();
    let mut source = "admin".to_string();

    while let Ok(Some(field)) = multipart.next_field().await {
        match field.name() {
            Some("caption") => {
                caption = field.text().await.ok();
            }
            Some("keep_original") => {
                keep_original = field.text().await.ok().as_deref() == Some("true");
            }
            Some("format") => {
                if let Ok(v) = field.text().await { format = v; }
            }
            Some("source") => {
                if let Ok(v) = field.text().await { source = v; }
            }
            Some("image") => {
                let content_type = field.content_type()
                    .unwrap_or("application/octet-stream")
                    .to_string();

                if !matches!(content_type.as_str(), "image/jpeg" | "image/png" | "image/webp") {
                    return (StatusCode::BAD_REQUEST, Html("Invalid image type".to_string()))
                        .into_response();
                }

                let bytes = field.bytes().await.unwrap_or_default();

                if bytes.len() > MAX_IMAGE_BYTES {
                    return (StatusCode::PAYLOAD_TOO_LARGE, Html("Image too large (max 35MB)".to_string()))
                        .into_response();
                }

                let ext = match validate_magic_bytes(&bytes) {
                    Some(ext) => ext,
                    None => return (StatusCode::BAD_REQUEST, Html("Invalid image file".to_string())).into_response(),
                };

                image_data = Some((bytes.to_vec(), format!("image/{ext}")));
            }
            _ => {}
        }
    }

    let caption = caption.unwrap_or_default();
    let (bytes, content_type) = match image_data {
        Some(d) => d,
        None => return (StatusCode::BAD_REQUEST, Html("Missing image".to_string())).into_response(),
    };
```

- [ ] **Step 7: Replace the upload + insert block with compression-aware version**

Replace everything from `// Generate unique key` to the final `Html(...).into_response()` with:

```rust
    // Compress if above threshold and user did not opt out
    let (final_bytes, final_content_type) = if !keep_original && bytes.len() > MAX_COMPRESS_BYTES {
        match compress_to_webp(&bytes) {
            Ok(webp) => {
                tracing::info!("compressed {} bytes -> {} bytes as webp", bytes.len(), webp.len());
                (webp, "image/webp".to_string())
            }
            Err(e) => {
                tracing::warn!("compression failed, storing original: {e}");
                (bytes, content_type)
            }
        }
    } else {
        (bytes, content_type)
    };

    let file_size_bytes = final_bytes.len() as i64;
    let ext = final_content_type.split('/').last().unwrap_or("jpg");
    let key = format!("{}.{}", uuid::Uuid::new_v4(), ext);

    let image_url = match state.storage.upload(&key, final_bytes, &final_content_type).await {
        Ok(url) => url,
        Err(e) => {
            tracing::error!("storage upload error: {e}");
            return (StatusCode::INTERNAL_SERVER_ERROR, Html("Upload failed".to_string())).into_response();
        }
    };

    let post = crate::db::insert_post(
        &state.pool, caption.trim(), &image_url, &format, file_size_bytes,
    ).await;
    tracing::info!("post created: id={}, key={key}, size={file_size_bytes} bytes, format={format}", post.id);

    let card_html = if source == "gallery" {
        crate::routes::feed::post_card_html(&post)
    } else {
        admin_post_card_html(&post)
    };
    Html(card_html).into_response()
```

- [ ] **Step 8: Run all tests**

```bash
cargo test 2>&1 | tail -20
```

Expected: all tests pass.

- [ ] **Step 9: Commit**

```bash
git add Cargo.toml src/routes/admin.rs
git commit -m "feat: compress images >4MB to WebP at upload with keep_original opt-out"
```

---

## Task 5: Gallery post composer

**Files:**
- Modify: `templates/artportfolio/feed.html`
- Modify: `static/style.css`

- [ ] **Step 1: Replace `templates/artportfolio/feed.html`**

```html
{% extends "base.html" %}

{% block title %}Drawing Portfolio{% endblock %}

{% block content %}
{% if is_admin %}
<div id="composer-wrap">
  <button id="new-post-btn" onclick="toggleComposer()" type="button" aria-expanded="false">+ New Post</button>
  <div id="composer" hidden>
    <div class="composer-formats">
      <button class="fmt-btn active" type="button">Single Image</button>
      <button class="fmt-btn" type="button" disabled title="Coming soon">Gallery</button>
      <button class="fmt-btn" type="button" disabled title="Coming soon">Board</button>
    </div>
    <form id="composer-form"
          hx-post="/api/admin/posts"
          hx-encoding="multipart/form-data"
          hx-target="#feed"
          hx-swap="afterbegin"
          hx-on::after-request="onComposerResponse(event)">
      <input type="hidden" name="format" value="single">
      <input type="hidden" name="source" value="gallery">
      <div class="drop-zone">
        <input type="file" name="image" id="composer-image"
               accept="image/jpeg,image/png,image/webp" required
               onchange="onFileSelect(this)">
        <label for="composer-image">Drop image here or click to browse</label>
        <span id="file-name" class="file-name-hint"></span>
      </div>
      <div id="compression-opt" hidden>
        <label class="compression-label">
          <input type="checkbox" name="keep_original" value="true">
          Keep original file without compression
        </label>
      </div>
      <textarea name="caption" placeholder="Caption (optional)" rows="2"></textarea>
      <div class="composer-actions">
        <button type="submit" class="btn-primary">Upload</button>
        <button type="button" class="btn-secondary" onclick="toggleComposer()">Cancel</button>
      </div>
    </form>
  </div>
</div>

<script>
const COMPRESS_THRESHOLD = 4 * 1024 * 1024;

function toggleComposer() {
  const composer = document.getElementById('composer');
  const btn = document.getElementById('new-post-btn');
  const willShow = composer.hidden;
  composer.hidden = !willShow;
  btn.setAttribute('aria-expanded', String(willShow));
}

function onFileSelect(input) {
  const file = input.files[0];
  document.getElementById('file-name').textContent = file ? file.name : '';
  document.getElementById('compression-opt').hidden = !(file && file.size > COMPRESS_THRESHOLD);
}

function onComposerResponse(evt) {
  if (evt.detail.successful) {
    document.getElementById('composer-form').reset();
    document.getElementById('file-name').textContent = '';
    document.getElementById('compression-opt').hidden = true;
  }
}
</script>
{% endif %}

<div id="feed"
     hx-get="/artportfolio/htmx/posts?page=0"
     hx-trigger="load"
     hx-swap="innerHTML">
  <p>Loading...</p>
</div>
{% endblock %}
```

- [ ] **Step 2: Append composer styles to `static/style.css`**

```css
/* ── Composer ──────────────────────────────────── */
#composer-wrap {
  margin-bottom: 1.5rem;
}

#new-post-btn {
  background: #333;
  color: #fff;
  border: none;
  border-radius: 20px;
  padding: 0.4rem 1.1rem;
  font-size: 0.9rem;
  cursor: pointer;
}

#new-post-btn:hover { background: #555; }

#composer {
  margin-top: 0.75rem;
  background: #fff;
  border: 1px solid #ddd;
  border-radius: 10px;
  padding: 1.25rem;
  max-width: 480px;
}

.composer-formats {
  display: flex;
  gap: 0.5rem;
  margin-bottom: 1rem;
}

.fmt-btn {
  border: 1px solid #ccc;
  border-radius: 14px;
  padding: 0.25rem 0.75rem;
  font-size: 0.8rem;
  cursor: pointer;
  background: #fff;
}

.fmt-btn.active {
  background: #333;
  color: #fff;
  border-color: #333;
}

.fmt-btn:disabled {
  color: #bbb;
  border-color: #e0e0e0;
  cursor: not-allowed;
}

.drop-zone {
  border: 2px dashed #ccc;
  border-radius: 6px;
  padding: 1rem;
  text-align: center;
  margin-bottom: 0.75rem;
}

.drop-zone input[type=file] { display: none; }

.drop-zone label {
  cursor: pointer;
  color: #666;
  font-size: 0.9rem;
}

.file-name-hint {
  display: block;
  font-size: 0.8rem;
  color: #555;
  margin-top: 0.3rem;
}

.compression-label {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  font-size: 0.85rem;
  color: #555;
  margin-bottom: 0.75rem;
}

#composer textarea {
  width: 100%;
  box-sizing: border-box;
  min-height: 60px;
  padding: 0.5rem;
  font: inherit;
  border: 1px solid #ccc;
  border-radius: 4px;
  resize: vertical;
  margin-bottom: 0.75rem;
}

.composer-actions { display: flex; gap: 0.5rem; }

.btn-primary {
  background: #333;
  color: #fff;
  border: none;
  border-radius: 4px;
  padding: 0.4rem 1.1rem;
  cursor: pointer;
}

.btn-primary:hover { background: #555; }

.btn-secondary {
  background: transparent;
  border: 1px solid #ccc;
  border-radius: 4px;
  padding: 0.4rem 0.9rem;
  cursor: pointer;
}
```

- [ ] **Step 3: Build and smoke-test**

```bash
cargo build 2>&1 | grep "^error"
```

Then `cargo run` and verify manually:
1. Visit `/artportfolio` logged out — no "+ New Post" button in page source
2. Log in, visit `/artportfolio` — "+ New Post" button is present
3. Click it — composer opens
4. Select file > 4MB — "Keep original file" checkbox appears
5. Select file < 4MB — checkbox stays hidden
6. Upload without caption — post appears in feed without caption paragraph
7. Upload with caption — post appears with caption

- [ ] **Step 4: Commit**

```bash
git add templates/artportfolio/feed.html static/style.css
git commit -m "feat: add floating post composer to gallery"
```

---

## Task 6: Command palette

**Files:**
- Create: `static/palette.js`
- Modify: `templates/base.html`
- Modify: `templates/admin.html`
- Modify: `static/style.css`

- [ ] **Step 1: Create `static/palette.js`**

```js
// Command palette — Ctrl+K / Cmd+K to open.
// To add a command: push one object to COMMANDS.
// adminOnly commands are hidden when IS_ADMIN is false.

const COMMANDS = [
  {
    label: 'Upload new drawing',
    keywords: ['upload', 'post', 'new', 'image', 'add'],
    adminOnly: true,
    action() {
      const composer = document.getElementById('composer');
      if (composer) {
        composer.hidden = false;
        document.getElementById('new-post-btn')?.setAttribute('aria-expanded', 'true');
      } else {
        location.href = '/artportfolio';
      }
    },
  },
  {
    label: 'Go to Art Portfolio',
    keywords: ['feed', 'gallery', 'art', 'drawings', 'portfolio'],
    action() { location.href = '/artportfolio'; },
  },
  {
    label: 'Go to Hub',
    keywords: ['home', 'hub', 'index', 'start', 'main'],
    action() { location.href = '/'; },
  },
  {
    label: 'Admin panel',
    keywords: ['admin', 'settings', 'manage'],
    adminOnly: true,
    action() { location.href = '/admin'; },
  },
];

// ── Engine ────────────────────────────────────────

let paletteResults = [];
let selectedIdx = 0;

function paletteAvailable() {
  return COMMANDS.filter(c => !c.adminOnly || (typeof IS_ADMIN !== 'undefined' && IS_ADMIN));
}

function paletteFilter(query) {
  const q = query.toLowerCase().trim();
  if (!q) return paletteAvailable();
  return paletteAvailable().filter(c =>
    c.label.toLowerCase().includes(q) ||
    c.keywords.some(k => k.includes(q))
  );
}

function paletteRender() {
  const list = document.getElementById('palette-results');
  if (!list) return;
  while (list.firstChild) list.removeChild(list.firstChild);

  paletteResults.forEach((cmd, i) => {
    const el = document.createElement('div');
    el.className = 'palette-item' + (i === selectedIdx ? ' palette-selected' : '');
    el.dataset.i = String(i);
    el.textContent = cmd.label;           // textContent — never innerHTML for dynamic data
    el.addEventListener('mousedown', e => {
      e.preventDefault();                  // prevent input blur before action fires
      selectedIdx = i;
      paletteExecute();
    });
    list.appendChild(el);
  });
}

function paletteExecute() {
  const cmd = paletteResults[selectedIdx];
  if (!cmd) return;
  paletteClose();
  cmd.action();
}

function paletteOpen() {
  const overlay = document.getElementById('palette-overlay');
  if (!overlay) return;
  overlay.hidden = false;
  const input = document.getElementById('palette-input');
  input.value = '';
  selectedIdx = 0;
  paletteResults = paletteAvailable();
  paletteRender();
  input.focus();
}

function paletteClose() {
  const overlay = document.getElementById('palette-overlay');
  if (overlay) overlay.hidden = true;
}

// ── Keyboard ──────────────────────────────────────

document.addEventListener('keydown', e => {
  const overlay = document.getElementById('palette-overlay');
  const isOpen = overlay && !overlay.hidden;

  if ((e.ctrlKey || e.metaKey) && e.key === 'k') {
    e.preventDefault();
    isOpen ? paletteClose() : paletteOpen();
    return;
  }
  if (!isOpen) return;

  if (e.key === 'Escape')    { e.preventDefault(); paletteClose(); return; }
  if (e.key === 'ArrowDown') { e.preventDefault(); selectedIdx = Math.min(selectedIdx + 1, paletteResults.length - 1); paletteRender(); return; }
  if (e.key === 'ArrowUp')   { e.preventDefault(); selectedIdx = Math.max(selectedIdx - 1, 0); paletteRender(); return; }
  if (e.key === 'Enter')     { e.preventDefault(); paletteExecute(); return; }
});

// ── DOM injection ─────────────────────────────────
// Build the overlay with DOM methods to avoid innerHTML with dynamic content.

document.addEventListener('DOMContentLoaded', () => {
  const overlay = document.createElement('div');
  overlay.id = 'palette-overlay';
  overlay.hidden = true;
  overlay.setAttribute('role', 'dialog');
  overlay.setAttribute('aria-modal', 'true');
  overlay.setAttribute('aria-label', 'Command palette');

  const box = document.createElement('div');
  box.id = 'palette-box';

  const input = document.createElement('input');
  input.id = 'palette-input';
  input.type = 'text';
  input.placeholder = 'Search commands\u2026';
  input.setAttribute('autocomplete', 'off');
  input.setAttribute('spellcheck', 'false');

  const results = document.createElement('div');
  results.id = 'palette-results';

  box.appendChild(input);
  box.appendChild(results);
  overlay.appendChild(box);
  document.body.appendChild(overlay);

  overlay.addEventListener('mousedown', e => {
    if (e.target === overlay) paletteClose();
  });

  input.addEventListener('input', e => {
    selectedIdx = 0;
    paletteResults = paletteFilter(e.target.value);
    paletteRender();
  });
});
```

- [ ] **Step 2: Add palette CSS to `static/style.css`**

```css
/* ── Command palette ───────────────────────────── */
#palette-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0,0,0,0.45);
  display: flex;
  align-items: flex-start;
  justify-content: center;
  padding-top: 18vh;
  z-index: 1000;
}

#palette-overlay[hidden] { display: none; }

#palette-box {
  background: #fff;
  border-radius: 10px;
  box-shadow: 0 8px 40px rgba(0,0,0,0.18);
  width: 100%;
  max-width: 480px;
  overflow: hidden;
}

#palette-input {
  width: 100%;
  box-sizing: border-box;
  border: none;
  border-bottom: 1px solid #e8e8e8;
  padding: 0.9rem 1rem;
  font-size: 1rem;
  outline: none;
}

#palette-results {
  max-height: 280px;
  overflow-y: auto;
}

.palette-item {
  padding: 0.65rem 1rem;
  cursor: pointer;
  font-size: 0.95rem;
  color: #222;
}

.palette-item:hover,
.palette-selected {
  background: #f0f0f0;
}
```

- [ ] **Step 3: Update `templates/base.html` to load palette.js**

Replace:
```html
  <script>const IS_ADMIN = {% if is_admin %}true{% else %}false{% endif %};</script>
  {% block head %}{% endblock %}
```

With:
```html
  <script>const IS_ADMIN = {% if is_admin %}true{% else %}false{% endif %};</script>
  <script src="/static/palette.js" defer></script>
  {% block head %}{% endblock %}
```

- [ ] **Step 4: Update `templates/admin.html` to include IS_ADMIN and palette.js**

In `templates/admin.html`, find the closing `</style>` tag and add these two lines immediately after it:
```html
  <script>const IS_ADMIN = true;</script>
  <script src="/static/palette.js" defer></script>
```

(Admin page is always auth-gated, so IS_ADMIN is always true here.)

- [ ] **Step 5: Build**

```bash
cargo build 2>&1 | grep "^error"
```

Expected: no errors.

- [ ] **Step 6: Smoke-test**

Run `cargo run` and verify:
1. Press Ctrl+K on `/` — palette opens, shows "Go to Art Portfolio" and "Go to Hub"; no admin commands
2. Log in, press Ctrl+K — all 4 commands appear
3. Type "art" — only "Go to Art Portfolio" shown; press Enter — navigates
4. Press Ctrl+K again — closes
5. Press Escape — closes
6. Click outside the palette box — closes
7. On `/artportfolio` logged in: Ctrl+K → "Upload new drawing" → composer opens
8. On `/admin`: Ctrl+K works and shows all commands

- [ ] **Step 7: Run full test suite**

```bash
cargo test 2>&1 | tail -20
```

Expected: all tests pass.

- [ ] **Step 8: Regenerate sqlx offline query data**

```bash
cargo sqlx prepare --database-url sqlite:./portfolio.db
```

Verify the `.sqlx/` directory has updated files, then stage them:
```bash
git add .sqlx/
```

- [ ] **Step 9: Final commit**

```bash
git add static/palette.js static/style.css templates/base.html templates/admin.html .sqlx/
git commit -m "feat: add command palette (Ctrl+K) with extensible command registry"
```

---

## Verification checklist

- [ ] `cargo test` — all tests green
- [ ] Upload with no caption — post card renders without `<p class="caption">`
- [ ] Upload PNG > 4MB, checkbox unchecked — stored URL ends in `.webp`
- [ ] Upload PNG > 4MB, "Keep original" checked — original format stored
- [ ] Upload file < 4MB — compression checkbox never appears
- [ ] Gallery "+ New Post" absent when logged out; present and functional when logged in
- [ ] Ctrl+K opens palette on `/`, `/artportfolio`, and `/admin`
- [ ] "Upload new drawing" hidden from palette when logged out
- [ ] `SQLX_OFFLINE=true cargo build --release` succeeds after `cargo sqlx prepare`
