# Drawing Portfolio Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a self-hosted Rust drawing portfolio website with a social-media feed style (image + caption posts), passkey-protected admin upload page, and Cloudflare R2 image storage.

**Architecture:** Single Axum 0.8 binary on a Hetzner VPS behind nginx. Server-side rendered HTML via Askama templates, HTMX for dynamic updates (server returns HTML fragments). SQLite stores posts/sessions/passkey credentials. Images live in Cloudflare R2 (public bucket).

**Tech Stack:** Rust/Axum 0.8, SQLite/sqlx 0.8, Askama 0.15 (manual `.render()` + `axum::response::Html`), HTMX, webauthn-rs 0.5, aws-sdk-s3 (Cloudflare R2), tower-http 0.6

---

## File Map

| File | Responsibility |
|---|---|
| `Cargo.toml` | All dependencies, pinned versions |
| `.env` / `.env.example` | Runtime config (R2 keys, domain, DB path) |
| `migrations/001_initial.sql` | SQLite schema |
| `src/main.rs` | Router, startup, background cleanup task |
| `src/db.rs` | sqlx pool + all SQL queries |
| `src/models.rs` | Post, Session, PasskeyCredential structs |
| `src/middleware.rs` | Session auth extractor, localhost-only guard |
| `src/storage.rs` | Cloudflare R2 upload + delete via aws-sdk-s3 |
| `src/routes/mod.rs` | Route module declarations |
| `src/routes/feed.rs` | GET /, GET /htmx/posts, GET /api/posts |
| `src/routes/admin.rs` | GET /admin, POST /api/admin/posts, DELETE /api/admin/posts/:id |
| `src/routes/auth.rs` | WebAuthn ceremony endpoints (register + login) |
| `templates/feed.html` | Full feed page (Askama) |
| `templates/admin.html` | Admin upload + manage page (Askama) |
| `templates/login.html` | Passkey login page (Askama) |
| `templates/partials/post_card.html` | Single post card (reused in feed + admin) |
| `static/style.css` | Plain CSS, no framework |
| `static/webauthn.js` | `navigator.credentials` calls + Base64url encoding |

---

## Phase 1: Project Skeleton

### Task 1: Dependencies and project structure

**Files:**
- Modify: `Cargo.toml`
- Create: `.gitignore`
- Create: `.env.example`
- Create: `src/main.rs`

- [ ] **Step 1: Update Cargo.toml with all dependencies**

```toml
[package]
name = "drawingportfolio"
version = "0.1.0"
edition = "2021"

[dependencies]
axum = { version = "0.8", features = ["multipart"] }
tokio = { version = "1", features = ["full"] }
sqlx = { version = "0.8", features = ["sqlite", "runtime-tokio", "macros"] }
askama = "0.15"
# Note: we use askama's .render() manually wrapped in axum::response::Html — no askama_web needed
webauthn-rs = { version = "0.5", features = ["danger-allow-state-serialisation"] }
aws-sdk-s3 = "1"
aws-config = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tower-http = { version = "0.6", features = ["fs", "limit"] }
tower = "0.5"
dotenvy = "0.15"
uuid = { version = "1", features = ["v4"] }
url = "2"
mime = "0.3"
tracing = "0.1"
tracing-subscriber = "0.3"
chrono = { version = "0.4.34", features = ["serde"] }  # pinned: 0.4.35+ renames Duration to TimeDelta
```

- [ ] **Step 2: Create .gitignore**

```
/target
.env
portfolio.db
portfolio.db-shm
portfolio.db-wal
```

- [ ] **Step 3: Create .env.example**

```
DATABASE_URL=sqlite:./portfolio.db
R2_ACCESS_KEY_ID=your_r2_access_key
R2_SECRET_ACCESS_KEY=your_r2_secret
R2_ACCOUNT_ID=your_cloudflare_account_id
R2_BUCKET=portfolio-images
R2_PUBLIC_URL=https://pub-xxx.r2.dev
RP_ID=localhost
RP_ORIGIN=http://localhost:3000
```

- [ ] **Step 4: Write minimal main.rs that compiles**

```rust
// src/main.rs
use axum::{Router, response::IntoResponse};
use tower_http::services::ServeDir;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    dotenvy::dotenv().ok();

    let app = Router::new()
        .nest_service("/static", ServeDir::new("static"))
        .fallback(handler_404);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    tracing::info!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

async fn handler_404() -> impl IntoResponse {
    (axum::http::StatusCode::NOT_FOUND, "not found")
}
```

- [ ] **Step 5: Verify it compiles and runs**

```bash
cd /home/hampter/projects/drawingportfolio
cargo build
```

Expected: compiles successfully (may take a few minutes first time)

```bash
cargo run
```

Expected: "listening on 0.0.0.0:3000" — Ctrl+C to stop.

- [ ] **Step 6: Create static and template directories**

```bash
mkdir -p static templates/partials src/routes migrations
touch static/style.css static/webauthn.js
```

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat: initial project setup with all dependencies"
```

---

## Phase 2: Database Layer

### Task 2: SQLite schema and query layer

**Files:**
- Create: `migrations/001_initial.sql`
- Create: `src/models.rs`
- Create: `src/db.rs`
- Modify: `src/main.rs` (add DB pool init)

- [ ] **Step 1: Write the migration file**

```sql
-- migrations/001_initial.sql
CREATE TABLE IF NOT EXISTS posts (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    caption    TEXT NOT NULL,
    image_url  TEXT NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS sessions (
    id         TEXT PRIMARY KEY,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    expires_at DATETIME NOT NULL
);

CREATE TABLE IF NOT EXISTS passkey_credentials (
    id            TEXT PRIMARY KEY,
    passkey_json  TEXT NOT NULL,
    created_at    DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS auth_challenge_state (
    id         TEXT PRIMARY KEY,
    state_json TEXT NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    expires_at DATETIME NOT NULL
);
```

- [ ] **Step 2: Write models.rs**

```rust
// src/models.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Post {
    pub id: i64,
    pub caption: String,
    pub image_url: String,
    pub created_at: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Session {
    pub id: String,
    pub expires_at: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PasskeyCredential {
    pub id: String,
    pub passkey_json: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AuthChallengeState {
    pub id: String,
    pub state_json: String,
}
```

- [ ] **Step 3: Write db.rs**

```rust
// src/db.rs
use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};
use crate::models::{Post, Session, PasskeyCredential, AuthChallengeState};

pub type DbPool = SqlitePool;

pub async fn connect(database_url: &str) -> DbPool {
    SqlitePoolOptions::new()
        .max_connections(5)
        .connect(database_url)
        .await
        .expect("failed to connect to SQLite")
}

pub async fn run_migrations(pool: &DbPool) {
    sqlx::query(include_str!("../migrations/001_initial.sql"))
        .execute(pool)
        .await
        .expect("failed to run migrations");
}

// Posts
pub async fn get_posts(pool: &DbPool, page: i64) -> Vec<Post> {
    let offset = page * 20;
    sqlx::query_as!(Post,
        "SELECT id, caption, image_url, created_at FROM posts ORDER BY created_at DESC LIMIT 21 OFFSET ?",
        offset
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default()
}

pub async fn insert_post(pool: &DbPool, caption: &str, image_url: &str) -> Post {
    let id = sqlx::query!(
        "INSERT INTO posts (caption, image_url) VALUES (?, ?) RETURNING id",
        caption, image_url
    )
    .fetch_one(pool)
    .await
    .expect("failed to insert post")
    .id;

    sqlx::query_as!(Post,
        "SELECT id, caption, image_url, created_at FROM posts WHERE id = ?", id
    )
    .fetch_one(pool)
    .await
    .expect("failed to fetch inserted post")
}

pub async fn delete_post_and_get_url(pool: &DbPool, id: i64) -> Option<String> {
    let row = sqlx::query!("SELECT image_url FROM posts WHERE id = ?", id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten();

    if let Some(r) = row {
        sqlx::query!("DELETE FROM posts WHERE id = ?", id)
            .execute(pool)
            .await
            .ok();
        Some(r.image_url)
    } else {
        None
    }
}

// Sessions
pub async fn create_session(pool: &DbPool, id: &str, expires_at: &str) {
    sqlx::query!(
        "INSERT INTO sessions (id, expires_at) VALUES (?, ?)",
        id, expires_at
    )
    .execute(pool)
    .await
    .expect("failed to create session");
}

pub async fn get_session(pool: &DbPool, id: &str) -> Option<Session> {
    sqlx::query_as!(Session,
        "SELECT id, expires_at FROM sessions WHERE id = ? AND expires_at > datetime('now')",
        id
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
}

pub async fn delete_session(pool: &DbPool, id: &str) {
    sqlx::query!("DELETE FROM sessions WHERE id = ?", id)
        .execute(pool)
        .await
        .ok();
}

pub async fn cleanup_expired(pool: &DbPool) {
    sqlx::query!("DELETE FROM sessions WHERE expires_at <= datetime('now')")
        .execute(pool)
        .await
        .ok();
    sqlx::query!("DELETE FROM auth_challenge_state WHERE expires_at <= datetime('now')")
        .execute(pool)
        .await
        .ok();
}

// Passkey credentials
pub async fn get_all_credentials(pool: &DbPool) -> Vec<PasskeyCredential> {
    sqlx::query_as!(PasskeyCredential,
        "SELECT id, passkey_json FROM passkey_credentials"
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default()
}

pub async fn save_credential(pool: &DbPool, id: &str, passkey_json: &str) {
    sqlx::query!(
        "INSERT OR REPLACE INTO passkey_credentials (id, passkey_json) VALUES (?, ?)",
        id, passkey_json
    )
    .execute(pool)
    .await
    .expect("failed to save credential");
}

// Auth challenge state
pub async fn save_challenge(pool: &DbPool, id: &str, state_json: &str, expires_at: &str) {
    sqlx::query!(
        "INSERT INTO auth_challenge_state (id, state_json, expires_at) VALUES (?, ?, ?)",
        id, state_json, expires_at
    )
    .execute(pool)
    .await
    .expect("failed to save challenge");
}

pub async fn take_challenge(pool: &DbPool, id: &str) -> Option<AuthChallengeState> {
    let row = sqlx::query_as!(AuthChallengeState,
        "SELECT id, state_json FROM auth_challenge_state WHERE id = ? AND expires_at > datetime('now')",
        id
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    if row.is_some() {
        sqlx::query!("DELETE FROM auth_challenge_state WHERE id = ?", id)
            .execute(pool)
            .await
            .ok();
    }

    row
}
```

- [ ] **Step 4: Update main.rs to initialize the DB pool**

Replace `src/main.rs` with:

```rust
// src/main.rs
mod db;
mod middleware;
mod models;
mod routes;
mod storage;

use std::sync::Arc;
use axum::Router;
use tower_http::services::ServeDir;

pub struct AppState {
    pub pool: db::DbPool,
    pub storage: storage::R2Storage,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    dotenvy::dotenv().ok();

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:./portfolio.db".to_string());

    let pool = db::connect(&database_url).await;
    db::run_migrations(&pool).await;

    let storage = storage::R2Storage::from_env().await;

    let state = Arc::new(AppState { pool, storage });

    // Background cleanup task
    {
        let pool = state.pool.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(3600));
            loop {
                interval.tick().await;
                db::cleanup_expired(&pool).await;
            }
        });
    }

    let app = Router::new()
        .merge(routes::feed::router())
        .merge(routes::admin::router())
        .merge(routes::auth::router())
        .nest_service("/static", ServeDir::new("static"))
        .with_state(state)
        .fallback(handler_404);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    tracing::info!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

async fn handler_404() -> impl axum::response::IntoResponse {
    (axum::http::StatusCode::NOT_FOUND, "not found")
}
```

- [ ] **Step 5: Set up sqlx offline mode**

`sqlx::query!` macros verify SQL against the database at compile time. You must either have `DATABASE_URL` set and a live DB, or use offline mode. The easiest approach:

```bash
# Install the sqlx CLI
cargo install sqlx-cli --no-default-features --features sqlite

# Create the database and run the migration
export DATABASE_URL=sqlite:./portfolio.db
sqlx database create
sqlx migrate run --source migrations

# Generate the offline query cache (commit the .sqlx/ directory)
cargo sqlx prepare
```

Add `.sqlx/` to git (it lets CI compile without a live DB). Also add `SQLX_OFFLINE=true` to `.env` for convenience:

```
SQLX_OFFLINE=true
```

- [ ] **Step 6: Create stub modules so it compiles**

Create `src/routes/mod.rs`:
```rust
pub mod admin;
pub mod auth;
pub mod feed;
```

Create `src/routes/feed.rs` (stub):
```rust
use axum::Router;
use std::sync::Arc;
use crate::AppState;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
}
```

Create `src/routes/admin.rs` (stub):
```rust
use axum::Router;
use std::sync::Arc;
use crate::AppState;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
}
```

Create `src/routes/auth.rs` (stub):
```rust
use axum::Router;
use std::sync::Arc;
use crate::AppState;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
}
```

Create `src/middleware.rs` (stub):
```rust
// session middleware — implemented in Task 6
```

Create `src/storage.rs` (stub — implemented in Task 4):
```rust
pub struct R2Storage;

impl R2Storage {
    pub async fn from_env() -> Self {
        R2Storage
    }
}
```

- [ ] **Step 7: Copy .env.example to .env and fill in localhost values**

```bash
cp .env.example .env
```

Edit `.env` — the defaults in `.env.example` work for local dev as-is.

- [ ] **Step 8: Run to verify DB initializes**

```bash
cargo run
```

Expected: starts without panic, `portfolio.db` file appears in project root.

- [ ] **Step 9: Write a basic DB test**

Add to the bottom of `src/db.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    async fn test_pool() -> DbPool {
        let pool = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .unwrap();
        run_migrations(&pool).await;
        pool
    }

    #[tokio::test]
    async fn test_insert_and_get_post() {
        let pool = test_pool().await;
        let post = insert_post(&pool, "test caption", "https://example.com/img.jpg").await;
        assert_eq!(post.caption, "test caption");

        let posts = get_posts(&pool, 0).await;
        assert_eq!(posts.len(), 1);
        assert_eq!(posts[0].id, post.id);
    }

    #[tokio::test]
    async fn test_delete_post() {
        let pool = test_pool().await;
        let post = insert_post(&pool, "to delete", "https://example.com/img.jpg").await;
        let url = delete_post_and_get_url(&pool, post.id).await;
        assert_eq!(url, Some("https://example.com/img.jpg".to_string()));
        assert!(get_posts(&pool, 0).await.is_empty());
    }

    #[tokio::test]
    async fn test_session_lifecycle() {
        let pool = test_pool().await;
        let id = "test-session-id";
        create_session(&pool, id, "2099-01-01T00:00:00").await;
        assert!(get_session(&pool, id).await.is_some());
        delete_session(&pool, id).await;
        assert!(get_session(&pool, id).await.is_none());
    }
}
```

- [ ] **Step 10: Run tests**

```bash
cargo test db::tests
```

Expected: 3 tests pass.

- [ ] **Step 11: Commit**

```bash
git add -A
git commit -m "feat: database layer with SQLite schema and queries"
```

---

## Phase 3: Feed Page

### Task 3: Public feed (HTML + HTMX + JSON API)

**Files:**
- Create: `templates/feed.html`
- Create: `templates/partials/post_card.html`
- Create: `static/style.css`
- Modify: `src/routes/feed.rs`

- [ ] **Step 1: Write the post card partial template**

Create `templates/partials/post_card.html`:
```html
<article class="post-card" id="post-{{ post.id }}">
  <img src="{{ post.image_url }}" alt="{{ post.caption }}" loading="lazy">
  <p class="caption">{{ post.caption }}</p>
  <small class="date">{{ post.created_at }}</small>
</article>
```

- [ ] **Step 2: Write the full feed page template**

Create `templates/feed.html`:
```html
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Portfolio</title>
  <link rel="stylesheet" href="/static/style.css">
  <script src="https://unpkg.com/htmx.org@2.0.4" integrity="sha384-HGfztofotfshcF7+8n44JQL2oJmowVChPTg48S+jvZoztPfvwD79OC/LTtG6dMp+" crossorigin="anonymous"></script>
</head>
<body>
  <header>
    <h1>Drawing Portfolio</h1>
  </header>
  <main>
    <div id="feed"
         hx-get="/htmx/posts?page=0"
         hx-trigger="load"
         hx-swap="innerHTML">
      <p>Loading...</p>
    </div>
  </main>
</body>
</html>
```

- [ ] **Step 3: Write basic CSS**

Create `static/style.css`:
```css
*, *::before, *::after { box-sizing: border-box; }

body {
  margin: 0;
  font-family: system-ui, sans-serif;
  background: #f5f5f5;
  color: #222;
}

header {
  background: #fff;
  border-bottom: 1px solid #ddd;
  padding: 1rem 2rem;
}

header h1 { margin: 0; font-size: 1.4rem; }

main {
  max-width: 640px;
  margin: 2rem auto;
  padding: 0 1rem;
}

#feed {
  display: flex;
  flex-direction: column;
  gap: 1.5rem;
}

.post-card {
  background: #fff;
  border: 1px solid #ddd;
  border-radius: 8px;
  overflow: hidden;
}

.post-card img {
  width: 100%;
  display: block;
  max-height: 600px;
  object-fit: contain;
  background: #000;
}

.caption {
  padding: 0.75rem 1rem 0.25rem;
  margin: 0;
}

.date {
  display: block;
  padding: 0 1rem 0.75rem;
  color: #888;
  font-size: 0.8rem;
}

.load-more {
  text-align: center;
  padding: 1rem;
}

.load-more button {
  padding: 0.5rem 1.5rem;
  cursor: pointer;
  border: 1px solid #ccc;
  border-radius: 4px;
  background: #fff;
}

.empty-state {
  text-align: center;
  padding: 4rem 1rem;
  color: #888;
}
```

- [ ] **Step 4: Write the feed routes**

Replace `src/routes/feed.rs`:

```rust
// src/routes/feed.rs
use axum::{
    Router,
    routing::get,
    extract::{Query, State},
    response::{Html, IntoResponse},
    Json,
};
use askama::Template;
use std::sync::Arc;
use serde::Deserialize;
use crate::{AppState, models::Post};

#[derive(Template)]
#[template(path = "feed.html")]
struct FeedTemplate;

#[derive(Deserialize)]
pub struct PageQuery {
    pub page: Option<i64>,
}

// Full page
async fn feed_page() -> impl IntoResponse {
    Html(FeedTemplate.render().unwrap())
}

// HTMX partial — returns HTML fragment
async fn htmx_posts(
    State(state): State<Arc<AppState>>,
    Query(q): Query<PageQuery>,
) -> impl IntoResponse {
    let page = q.page.unwrap_or(0);
    let mut posts = crate::db::get_posts(&state.pool, page).await;
    let has_more = posts.len() > 20;
    if has_more { posts.truncate(20); }

    let next_page = page + 1;
    let mut html = String::new();

    if posts.is_empty() && page == 0 {
        html.push_str(r#"<div class="empty-state"><p>No posts yet.</p></div>"#);
    } else {
        for post in &posts {
            html.push_str(&post_card_html(post));
        }
        if has_more {
            html.push_str(&format!(
                r#"<div class="load-more" id="load-more">
                  <button hx-get="/htmx/posts?page={next_page}"
                          hx-target="#load-more"
                          hx-swap="outerHTML">
                    Load more
                  </button>
                </div>"#
            ));
        }
    }

    Html(html)
}

// Public JSON API
#[derive(serde::Serialize)]
struct PostsResponse {
    posts: Vec<Post>,
    has_more: bool,
}

async fn api_posts(
    State(state): State<Arc<AppState>>,
    Query(q): Query<PageQuery>,
) -> impl IntoResponse {
    let page = q.page.unwrap_or(0);
    let mut posts = crate::db::get_posts(&state.pool, page).await;
    let has_more = posts.len() > 20;
    if has_more { posts.truncate(20); }
    Json(PostsResponse { posts, has_more })
}

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

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
     .replace('<', "&lt;")
     .replace('>', "&gt;")
     .replace('"', "&quot;")
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(feed_page))
        .route("/htmx/posts", get(htmx_posts))
        .route("/api/posts", get(api_posts))
}
```

- [ ] **Step 5: Verify it compiles**

```bash
cargo build
```

Expected: compiles without errors.

- [ ] **Step 6: Manual test**

```bash
cargo run
```

Open `http://localhost:3000` — feed page loads with "No posts yet." message.

Open `http://localhost:3000/api/posts` — returns `{"posts":[],"has_more":false}`.

- [ ] **Step 7: Write a route test**

Add to `src/routes/feed.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;
    use crate::{db, storage};

    async fn test_app() -> Router {
        let pool = db::SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .unwrap();
        db::run_migrations(&pool).await;
        let storage = storage::R2Storage::from_env().await;
        let state = Arc::new(crate::AppState { pool, storage });
        router().with_state(state)
    }

    #[tokio::test]
    async fn test_api_posts_empty() {
        let app = test_app().await;
        let resp = app
            .oneshot(Request::builder().uri("/api/posts").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }
}
```

Note: this test requires `use sqlx::sqlite::SqlitePoolOptions;` to be added to `db.rs` (make it `pub use`).

- [ ] **Step 8: Run tests**

```bash
cargo test routes::feed::tests
```

Expected: 1 test passes.

- [ ] **Step 9: Commit**

```bash
git add -A
git commit -m "feat: public feed page with HTMX posts and JSON API"
```

---

## Phase 4: Image Storage

### Task 4: Cloudflare R2 storage integration

**Files:**
- Modify: `src/storage.rs`

- [ ] **Step 1: Implement R2Storage**

Replace `src/storage.rs`:

```rust
// src/storage.rs
use aws_sdk_s3::{Client, config::Region, primitives::ByteStream};
use aws_config::BehaviorVersion;

pub struct R2Storage {
    client: Client,
    bucket: String,
    public_url: String,
}

impl R2Storage {
    pub async fn from_env() -> Self {
        let account_id = std::env::var("R2_ACCOUNT_ID")
            .unwrap_or_else(|_| "local".to_string());
        let bucket = std::env::var("R2_BUCKET")
            .unwrap_or_else(|_| "portfolio-images".to_string());
        let public_url = std::env::var("R2_PUBLIC_URL")
            .unwrap_or_else(|_| "http://localhost:3000/static".to_string());

        let endpoint = format!("https://{}.r2.cloudflarestorage.com", account_id);

        let config = aws_config::defaults(BehaviorVersion::latest())
            .endpoint_url(&endpoint)
            .region(Region::new("auto"))
            .load()
            .await;

        let s3_config = aws_sdk_s3::config::Builder::from(&config)
            .force_path_style(true)
            .build();

        let client = Client::from_conf(s3_config);

        R2Storage { client, bucket, public_url }
    }

    /// Upload bytes, return the public URL for the stored object.
    pub async fn upload(&self, key: &str, data: Vec<u8>, content_type: &str) -> Result<String, String> {
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .body(ByteStream::from(data))
            .content_type(content_type)
            .send()
            .await
            .map_err(|e| format!("R2 upload failed: {e}"))?;

        Ok(format!("{}/{}", self.public_url.trim_end_matches('/'), key))
    }

    /// Delete an object by its key (extracted from its public URL).
    pub async fn delete_by_url(&self, image_url: &str) -> Result<(), String> {
        let key = image_url
            .trim_start_matches(self.public_url.trim_end_matches('/'))
            .trim_start_matches('/');

        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .map_err(|e| format!("R2 delete failed: {e}"))?;

        Ok(())
    }
}
```

- [ ] **Step 2: Verify it compiles**

```bash
cargo build
```

Expected: compiles without errors. (R2 operations won't work locally without real credentials — that's fine.)

- [ ] **Step 3: Commit**

```bash
git add src/storage.rs
git commit -m "feat: Cloudflare R2 storage integration via aws-sdk-s3"
```

---

## Phase 5: Admin Routes

### Task 5: Admin upload page and post management

**Files:**
- Create: `templates/admin.html`
- Modify: `src/routes/admin.rs`

- [ ] **Step 1: Write the admin template**

Create `templates/admin.html`:
```html
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Admin — Portfolio</title>
  <link rel="stylesheet" href="/static/style.css">
  <script src="https://unpkg.com/htmx.org@2.0.4" integrity="sha384-HGfztofotfshcF7+8n44JQL2oJmowVChPTg48S+jvZoztPfvwD79OC/LTtG6dMp+" crossorigin="anonymous"></script>
  <style>
    .admin-form { background: #fff; border: 1px solid #ddd; border-radius: 8px; padding: 1.5rem; margin-bottom: 2rem; }
    .admin-form textarea { width: 100%; min-height: 80px; padding: 0.5rem; margin: 0.5rem 0; font: inherit; border: 1px solid #ccc; border-radius: 4px; }
    .admin-form input[type=file] { margin: 0.5rem 0; }
    .admin-form button { padding: 0.5rem 1.5rem; background: #333; color: #fff; border: none; border-radius: 4px; cursor: pointer; }
    .admin-post { display: flex; gap: 1rem; align-items: flex-start; background: #fff; border: 1px solid #ddd; border-radius: 8px; padding: 1rem; }
    .admin-post img { width: 80px; height: 80px; object-fit: cover; border-radius: 4px; }
    .admin-post .info { flex: 1; }
    .delete-btn { background: #c00; color: #fff; border: none; padding: 0.25rem 0.75rem; border-radius: 4px; cursor: pointer; }
    #upload-status { margin-top: 0.5rem; color: #555; }
  </style>
</head>
<body>
  <header>
    <h1>Portfolio Admin</h1>
    <a href="/">View site</a>
    <form action="/api/auth/logout" method="post" style="display:inline">
      <button type="submit">Log out</button>
    </form>
  </header>
  <main>
    <section class="admin-form">
      <h2>New Post</h2>
      <form id="upload-form"
            hx-post="/api/admin/posts"
            hx-encoding="multipart/form-data"
            hx-target="#posts-list"
            hx-swap="afterbegin"
            hx-on::after-request="this.reset()">
        <label>Caption<br>
          <textarea name="caption" required placeholder="Write something..."></textarea>
        </label><br>
        <label>Image<br>
          <input type="file" name="image" accept="image/jpeg,image/png,image/webp" required>
        </label><br>
        <button type="submit">Upload</button>
        <div id="upload-status"></div>
      </form>
    </section>

    <section>
      <h2>Posts</h2>
      <div id="posts-list"
           hx-get="/htmx/admin/posts"
           hx-trigger="load">
        Loading...
      </div>
    </section>
  </main>
</body>
</html>
```

- [ ] **Step 2: Implement admin routes**

Replace `src/routes/admin.rs`:

```rust
// src/routes/admin.rs
use axum::{
    Router,
    routing::{get, post, delete},
    extract::{State, Path, Multipart},
    response::{Html, IntoResponse, Redirect},
    http::StatusCode,
};
use askama::Template;
use std::sync::Arc;
use crate::AppState;

const MAX_IMAGE_BYTES: usize = 10 * 1024 * 1024; // 10 MB

#[derive(Template)]
#[template(path = "admin.html")]
struct AdminTemplate;

async fn admin_page() -> impl IntoResponse {
    Html(AdminTemplate.render().unwrap())
}

// HTMX partial — list of posts for admin view
async fn htmx_admin_posts(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let posts = crate::db::get_posts(&state.pool, 0).await;
    let mut html = String::new();
    for post in &posts {
        html.push_str(&admin_post_card_html(post));
    }
    if html.is_empty() {
        html = "<p>No posts yet.</p>".to_string();
    }
    Html(html)
}

async fn upload_post(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let mut caption = None::<String>;
    let mut image_data = None::<(Vec<u8>, String)>; // (bytes, content_type)

    while let Ok(Some(field)) = multipart.next_field().await {
        match field.name() {
            Some("caption") => {
                caption = field.text().await.ok();
            }
            Some("image") => {
                let content_type = field.content_type()
                    .unwrap_or("application/octet-stream")
                    .to_string();

                // Validate MIME type
                if !matches!(content_type.as_str(), "image/jpeg" | "image/png" | "image/webp") {
                    return (StatusCode::BAD_REQUEST, Html("Invalid image type".to_string()))
                        .into_response();
                }

                let bytes = field.bytes().await.unwrap_or_default();

                if bytes.len() > MAX_IMAGE_BYTES {
                    return (StatusCode::PAYLOAD_TOO_LARGE, Html("Image too large (max 10MB)".to_string()))
                        .into_response();
                }

                // Validate magic bytes
                let ext = match validate_magic_bytes(&bytes) {
                    Some(ext) => ext,
                    None => return (StatusCode::BAD_REQUEST, Html("Invalid image file".to_string())).into_response(),
                };

                image_data = Some((bytes.to_vec(), format!("image/{}", ext)));
            }
            _ => {}
        }
    }

    let (caption, (bytes, content_type)) = match (caption, image_data) {
        (Some(c), Some(d)) if !c.trim().is_empty() => (c, d),
        _ => return (StatusCode::BAD_REQUEST, Html("Missing caption or image".to_string())).into_response(),
    };

    // Generate unique key
    let key = format!("{}.{}", uuid::Uuid::new_v4(), content_type.split('/').last().unwrap_or("jpg"));

    let image_url = match state.storage.upload(&key, bytes, &content_type).await {
        Ok(url) => url,
        Err(e) => {
            tracing::error!("R2 upload error: {e}");
            return (StatusCode::INTERNAL_SERVER_ERROR, Html("Upload failed".to_string())).into_response();
        }
    };

    let post = crate::db::insert_post(&state.pool, caption.trim(), &image_url).await;
    Html(admin_post_card_html(&post)).into_response()
}

async fn delete_post(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    if let Some(image_url) = crate::db::delete_post_and_get_url(&state.pool, id).await {
        state.storage.delete_by_url(&image_url).await.ok();
    }
    StatusCode::OK
}

fn validate_magic_bytes(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) { return Some("jpeg"); }
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") { return Some("png"); }
    if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") { return Some("webp"); }
    None
}

fn admin_post_card_html(post: &crate::models::Post) -> String {
    format!(
        r#"<div class="admin-post" id="admin-post-{}">
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
</div>"#,
        post.id,
        html_escape(&post.image_url),
        html_escape(&post.caption),
        html_escape(&post.created_at),
        post.id,
        post.id,
    )
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
     .replace('<', "&lt;")
     .replace('>', "&gt;")
     .replace('"', "&quot;")
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/admin", get(admin_page))
        .route("/htmx/admin/posts", get(htmx_admin_posts))
        .route("/api/admin/posts", post(upload_post))
        .route("/api/admin/posts/{id}", delete(delete_post))
}
```

- [ ] **Step 3: Compile and test manually**

```bash
cargo build
cargo run
```

Visit `http://localhost:3000/admin` — admin page loads with upload form.

(Upload won't work yet without real R2 credentials — that's expected. The form and page should render.)

- [ ] **Step 4: Write upload validation test**

Add to `src/routes/admin.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_magic_bytes_jpeg() {
        let jpeg = vec![0xFF, 0xD8, 0xFF, 0xE0];
        assert_eq!(validate_magic_bytes(&jpeg), Some("jpeg"));
    }

    #[test]
    fn test_magic_bytes_png() {
        let png = b"\x89PNG\r\n\x1a\nrest".to_vec();
        assert_eq!(validate_magic_bytes(&png), Some("png"));
    }

    #[test]
    fn test_magic_bytes_invalid() {
        let bad = vec![0x00, 0x01, 0x02];
        assert_eq!(validate_magic_bytes(&bad), None);
    }
}
```

- [ ] **Step 5: Run tests**

```bash
cargo test routes::admin::tests
```

Expected: 3 tests pass.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat: admin upload page with R2 integration and image validation"
```

---

## Phase 6: Session Middleware

### Task 6: Session extraction and localhost guard

**Files:**
- Modify: `src/middleware.rs`
- Modify: `src/routes/admin.rs` (add auth guard)
- Modify: `src/routes/auth.rs` (add logout stub)

- [ ] **Step 1: Implement middleware**

Replace `src/middleware.rs`:

```rust
// src/middleware.rs
use axum::{
    extract::{FromRequestParts, State, ConnectInfo},
    http::{request::Parts, StatusCode},
    response::{IntoResponse, Redirect},
};
use std::{net::SocketAddr, sync::Arc};
use crate::{AppState, db};

/// Extractor: requires a valid session cookie. Redirects to /admin/login if missing/expired.
pub struct AuthSession(pub String); // session ID

impl FromRequestParts<Arc<AppState>> for AuthSession {
    type Rejection = axum::response::Response;

    async fn from_request_parts(parts: &mut Parts, state: &Arc<AppState>) -> Result<Self, Self::Rejection> {
        let session_id = extract_session_cookie(parts);

        if let Some(id) = session_id {
            if db::get_session(&state.pool, &id).await.is_some() {
                return Ok(AuthSession(id));
            }
        }

        Err(Redirect::to("/admin/login").into_response())
    }
}

fn extract_session_cookie(parts: &Parts) -> Option<String> {
    let cookies = parts.headers.get("cookie")?.to_str().ok()?;
    for cookie in cookies.split(';') {
        let cookie = cookie.trim();
        if let Some(val) = cookie.strip_prefix("session=") {
            return Some(val.to_string());
        }
    }
    None
}

/// Extractor: only allows requests from localhost (raw socket address).
pub struct LocalhostOnly;

impl<S: Send + Sync> FromRequestParts<S> for LocalhostOnly {
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        // ConnectInfo is set by axum::serve
        let addr = parts.extensions.get::<ConnectInfo<SocketAddr>>()
            .map(|ci| ci.0);

        match addr {
            Some(addr) if addr.ip().is_loopback() => Ok(LocalhostOnly),
            _ => Err(StatusCode::FORBIDDEN),
        }
    }
}

pub fn make_session_cookie(id: &str) -> String {
    format!("session={id}; HttpOnly; SameSite=Strict; Max-Age=2592000; Path=/")
}
```

- [ ] **Step 2: Protect admin routes with AuthSession**

Update `src/routes/admin.rs` — add `_session: AuthSession` parameter to handlers that need auth:

```rust
// Update admin_page signature:
async fn admin_page(_session: crate::middleware::AuthSession) -> impl IntoResponse {
    Html(AdminTemplate.render().unwrap())
}

// Update htmx_admin_posts signature:
async fn htmx_admin_posts(
    _session: crate::middleware::AuthSession,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse { ... }

// Update upload_post signature:
async fn upload_post(
    _session: crate::middleware::AuthSession,
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> impl IntoResponse { ... }

// Update delete_post signature:
async fn delete_post(
    _session: crate::middleware::AuthSession,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse { ... }
```

- [ ] **Step 3: Update main.rs to use ConnectInfo**

In `main.rs`, update the `axum::serve` call to include `ConnectInfo`:

```rust
axum::serve(
    listener,
    app.into_make_service_with_connect_info::<SocketAddr>(),
).await.unwrap();
```

- [ ] **Step 4: Add logout stub to auth.rs**

Update `src/routes/auth.rs` to handle logout:

```rust
// src/routes/auth.rs
use axum::{
    Router,
    routing::post,
    extract::State,
    response::{IntoResponse, Redirect},
    http::{HeaderMap, StatusCode},
};
use std::sync::Arc;
use crate::AppState;

async fn logout(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    // Extract session cookie and delete it
    if let Some(cookies) = headers.get("cookie").and_then(|v| v.to_str().ok()) {
        for cookie in cookies.split(';') {
            if let Some(id) = cookie.trim().strip_prefix("session=") {
                crate::db::delete_session(&state.pool, id).await;
            }
        }
    }
    let mut response_headers = axum::http::HeaderMap::new();
    response_headers.insert(
        axum::http::header::SET_COOKIE,
        "session=; HttpOnly; SameSite=Strict; Max-Age=0; Path=/".parse().unwrap(),
    );
    response_headers.insert(
        axum::http::header::LOCATION,
        "/admin/login".parse().unwrap(),
    );
    (StatusCode::SEE_OTHER, response_headers).into_response()
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/auth/logout", post(logout))
        // WebAuthn routes added in Task 7
}
```

- [ ] **Step 5: Compile**

```bash
cargo build
```

Expected: compiles. Visiting `/admin` should now redirect to `/admin/login` (which returns 404 for now — that's fine until Task 7).

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat: session middleware and auth protection on admin routes"
```

---

## Phase 7: WebAuthn Authentication

### Task 7: Passkey registration and login

**Files:**
- Create: `templates/login.html`
- Create: `static/webauthn.js`
- Modify: `src/routes/auth.rs`
- Modify: `src/main.rs` (add WebAuthn state)

- [ ] **Step 1: Add WebAuthn to AppState**

Update `src/main.rs`:

```rust
// Add to imports
use webauthn_rs::prelude::*;
use url::Url;

pub struct AppState {
    pub pool: db::DbPool,
    pub storage: storage::R2Storage,
    pub webauthn: Webauthn,
}

// In main(), before building state:
let rp_id = std::env::var("RP_ID").unwrap_or_else(|_| "localhost".to_string());
let rp_origin = std::env::var("RP_ORIGIN")
    .unwrap_or_else(|_| "http://localhost:3000".to_string());
let rp_origin_url = Url::parse(&rp_origin).expect("invalid RP_ORIGIN");

let webauthn = WebauthnBuilder::new(&rp_id, &rp_origin_url)
    .expect("invalid WebAuthn config")
    .rp_name("Drawing Portfolio")
    .build()
    .expect("failed to build WebAuthn");

let state = Arc::new(AppState { pool, storage, webauthn });
```

- [ ] **Step 2: Write the login page template**

Create `templates/login.html`:
```html
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Login — Portfolio</title>
  <link rel="stylesheet" href="/static/style.css">
  <style>
    .login-box { max-width: 360px; margin: 4rem auto; padding: 2rem; background: #fff; border: 1px solid #ddd; border-radius: 8px; text-align: center; }
    .login-box button { padding: 0.75rem 2rem; background: #333; color: #fff; border: none; border-radius: 4px; cursor: pointer; font-size: 1rem; }
    #status { margin-top: 1rem; color: #c00; }
  </style>
</head>
<body>
  <main>
    <div class="login-box">
      <h1>Portfolio Admin</h1>
      <p>Log in with your passkey</p>
      <button id="login-btn" onclick="startLogin()">Log in with passkey</button>
      <div id="status"></div>
    </div>
  </main>
  <script src="/static/webauthn.js"></script>
</body>
</html>
```

- [ ] **Step 3: Write webauthn.js**

Create `static/webauthn.js`:
```javascript
// Encode ArrayBuffer to Base64url string
function bufToB64url(buf) {
  return btoa(String.fromCharCode(...new Uint8Array(buf)))
    .replace(/\+/g, '-').replace(/\//g, '_').replace(/=/g, '');
}

// Decode Base64url string to Uint8Array
function b64urlToBuf(str) {
  const b64 = str.replace(/-/g, '+').replace(/_/g, '/');
  const bin = atob(b64);
  return Uint8Array.from(bin, c => c.charCodeAt(0));
}

async function startLogin() {
  const status = document.getElementById('status');
  status.textContent = '';

  try {
    // Step 1: Get challenge from server
    const startResp = await fetch('/api/auth/login/start', { method: 'POST' });
    const { challenge_id, options } = await startResp.json();

    // Decode challenge and allowCredentials buffers
    options.publicKey.challenge = b64urlToBuf(options.publicKey.challenge);
    if (options.publicKey.allowCredentials) {
      options.publicKey.allowCredentials = options.publicKey.allowCredentials.map(c => ({
        ...c, id: b64urlToBuf(c.id)
      }));
    }

    // Step 2: Ask device for passkey
    const credential = await navigator.credentials.get(options);

    // Step 3: Encode response and send to server
    const credJson = {
      id: credential.id,
      rawId: bufToB64url(credential.rawId),
      type: credential.type,
      response: {
        clientDataJSON: bufToB64url(credential.response.clientDataJSON),
        authenticatorData: bufToB64url(credential.response.authenticatorData),
        signature: bufToB64url(credential.response.signature),
        userHandle: credential.response.userHandle
          ? bufToB64url(credential.response.userHandle) : null,
      },
    };

    const finishResp = await fetch('/api/auth/login/finish', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ challenge_id, credential: credJson }),
    });
    const result = await finishResp.json();

    if (result.ok) {
      window.location.href = '/admin';
    } else {
      status.textContent = result.error || 'Login failed';
    }
  } catch (err) {
    status.textContent = err.message || 'Login failed';
  }
}

async function startRegister() {
  const status = document.getElementById('status');
  status.textContent = '';

  try {
    const startResp = await fetch('/api/auth/register/start', { method: 'POST' });
    const { challenge_id, options } = await startResp.json();

    options.publicKey.challenge = b64urlToBuf(options.publicKey.challenge);
    options.publicKey.user.id = b64urlToBuf(options.publicKey.user.id);
    if (options.publicKey.excludeCredentials) {
      options.publicKey.excludeCredentials = options.publicKey.excludeCredentials.map(c => ({
        ...c, id: b64urlToBuf(c.id)
      }));
    }

    const credential = await navigator.credentials.create(options);

    const credJson = {
      id: credential.id,
      rawId: bufToB64url(credential.rawId),
      type: credential.type,
      response: {
        clientDataJSON: bufToB64url(credential.response.clientDataJSON),
        attestationObject: bufToB64url(credential.response.attestationObject),
      },
    };

    const finishResp = await fetch('/api/auth/register/finish', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ challenge_id, credential: credJson }),
    });
    const result = await finishResp.json();

    if (result.ok) {
      status.textContent = 'Passkey registered! You can now log in.';
      status.style.color = 'green';
    } else {
      status.textContent = result.error || 'Registration failed';
    }
  } catch (err) {
    status.textContent = err.message || 'Registration failed';
  }
}
```

- [ ] **Step 4: Implement auth routes**

Replace `src/routes/auth.rs`:

```rust
// src/routes/auth.rs
use axum::{
    Router,
    routing::{get, post},
    extract::State,
    response::{Html, IntoResponse, Redirect},
    http::{StatusCode, HeaderMap, HeaderValue},
    Json,
};
use askama::Template;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;
use webauthn_rs::prelude::*;
use crate::{AppState, db, middleware};

#[derive(Template)]
#[template(path = "login.html")]
struct LoginTemplate;

async fn login_page() -> impl IntoResponse {
    Html(LoginTemplate.render().unwrap())
}

// Registration (localhost only)

#[derive(Serialize)]
struct StartResponse {
    challenge_id: String,
    options: serde_json::Value,
}

#[derive(Deserialize)]
struct FinishBody {
    challenge_id: String,
    credential: serde_json::Value,
}

async fn register_start(
    _: crate::middleware::LocalhostOnly,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let user_id = Uuid::new_v4();
    let user_unique_id = user_id.as_bytes().to_vec();

    // Get existing credentials to exclude
    let existing: Vec<CredentialID> = db::get_all_credentials(&state.pool)
        .await
        .iter()
        .filter_map(|c| serde_json::from_str::<Passkey>(&c.passkey_json).ok())
        .map(|pk| pk.cred_id().clone())
        .collect();

    match state.webauthn.start_passkey_registration(
        user_id,
        "admin",
        "Portfolio Admin",
        Some(existing),
    ) {
        Ok((ccr, reg_state)) => {
            let challenge_id = Uuid::new_v4().to_string();
            let state_json = serde_json::to_string(&reg_state).unwrap();
            let expires = chrono::Utc::now()
                .checked_add_signed(chrono::Duration::minutes(5))
                .unwrap()
                .format("%Y-%m-%dT%H:%M:%S")
                .to_string();
            db::save_challenge(&state.pool, &challenge_id, &state_json, &expires).await;
            let options = serde_json::to_value(&ccr).unwrap();
            Json(serde_json::json!({ "challenge_id": challenge_id, "options": options }))
                .into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR,
                   Json(serde_json::json!({ "ok": false, "error": e.to_string() }))).into_response(),
    }
}

async fn register_finish(
    _: crate::middleware::LocalhostOnly,
    State(state): State<Arc<AppState>>,
    Json(body): Json<FinishBody>,
) -> impl IntoResponse {
    let challenge = match db::take_challenge(&state.pool, &body.challenge_id).await {
        Some(c) => c,
        None => return Json(serde_json::json!({ "ok": false, "error": "challenge expired or invalid" })).into_response(),
    };

    let reg_state: PasskeyRegistration = match serde_json::from_str(&challenge.state_json) {
        Ok(s) => s,
        Err(_) => return Json(serde_json::json!({ "ok": false, "error": "invalid challenge state" })).into_response(),
    };

    let reg_response: RegisterPublicKeyCredential = match serde_json::from_value(body.credential) {
        Ok(r) => r,
        Err(e) => return Json(serde_json::json!({ "ok": false, "error": e.to_string() })).into_response(),
    };

    match state.webauthn.finish_passkey_registration(&reg_response, &reg_state) {
        Ok(passkey) => {
            // cred_id() returns Base64UrlSafeData which serialises to a string via serde_json
            let cred_id = serde_json::to_value(passkey.cred_id())
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
            let passkey_json = serde_json::to_string(&passkey).unwrap();
            db::save_credential(&state.pool, &cred_id, &passkey_json).await;
            Json(serde_json::json!({ "ok": true })).into_response()
        }
        Err(e) => Json(serde_json::json!({ "ok": false, "error": e.to_string() })).into_response(),
    }
}

// Login

async fn login_start(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let passkeys: Vec<Passkey> = db::get_all_credentials(&state.pool)
        .await
        .iter()
        .filter_map(|c| serde_json::from_str(&c.passkey_json).ok())
        .collect();

    if passkeys.is_empty() {
        return (StatusCode::FORBIDDEN,
                Json(serde_json::json!({ "ok": false, "error": "no passkeys registered" }))).into_response();
    }

    match state.webauthn.start_passkey_authentication(&passkeys) {
        Ok((rcr, auth_state)) => {
            let challenge_id = Uuid::new_v4().to_string();
            let state_json = serde_json::to_string(&auth_state).unwrap();
            let expires = chrono::Utc::now()
                .checked_add_signed(chrono::Duration::minutes(5))
                .unwrap()
                .format("%Y-%m-%dT%H:%M:%S")
                .to_string();
            db::save_challenge(&state.pool, &challenge_id, &state_json, &expires).await;
            let options = serde_json::to_value(&rcr).unwrap();
            Json(serde_json::json!({ "challenge_id": challenge_id, "options": options })).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR,
                   Json(serde_json::json!({ "ok": false, "error": e.to_string() }))).into_response(),
    }
}

async fn login_finish(
    State(state): State<Arc<AppState>>,
    Json(body): Json<FinishBody>,
) -> impl IntoResponse {
    let challenge = match db::take_challenge(&state.pool, &body.challenge_id).await {
        Some(c) => c,
        None => return Json(serde_json::json!({ "ok": false, "error": "challenge expired" })).into_response(),
    };

    let auth_state: PasskeyAuthentication = match serde_json::from_str(&challenge.state_json) {
        Ok(s) => s,
        Err(_) => return Json(serde_json::json!({ "ok": false, "error": "invalid state" })).into_response(),
    };

    let auth_response: PublicKeyCredential = match serde_json::from_value(body.credential) {
        Ok(r) => r,
        Err(e) => return Json(serde_json::json!({ "ok": false, "error": e.to_string() })).into_response(),
    };

    match state.webauthn.finish_passkey_authentication(&auth_response, &auth_state) {
        Ok(_result) => {
            // Session fixation prevention: always generate fresh session ID
            let session_id = Uuid::new_v4().to_string();
            let expires = chrono::Utc::now()
                .checked_add_signed(chrono::Duration::days(30))
                .unwrap()
                .format("%Y-%m-%dT%H:%M:%S")
                .to_string();
            db::create_session(&state.pool, &session_id, &expires).await;

            let cookie = middleware::make_session_cookie(&session_id);
            let mut headers = axum::http::HeaderMap::new();
            headers.insert(
                axum::http::header::SET_COOKIE,
                cookie.parse().unwrap(),
            );
            (headers, Json(serde_json::json!({ "ok": true }))).into_response()
        }
        Err(e) => Json(serde_json::json!({ "ok": false, "error": e.to_string() })).into_response(),
    }
}

async fn logout(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if let Some(cookies) = headers.get("cookie").and_then(|v| v.to_str().ok()) {
        for cookie in cookies.split(';') {
            if let Some(id) = cookie.trim().strip_prefix("session=") {
                db::delete_session(&state.pool, id).await;
            }
        }
    }
    let mut resp_headers = axum::http::HeaderMap::new();
    resp_headers.insert(
        axum::http::header::SET_COOKIE,
        "session=; HttpOnly; SameSite=Strict; Max-Age=0; Path=/".parse().unwrap(),
    );
    resp_headers.insert(
        axum::http::header::LOCATION,
        "/admin/login".parse().unwrap(),
    );
    (StatusCode::SEE_OTHER, resp_headers).into_response()
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/admin/login", get(login_page))
        .route("/api/auth/login/start", post(login_start))
        .route("/api/auth/login/finish", post(login_finish))
        .route("/api/auth/logout", post(logout))
        .route("/api/auth/register/start", post(register_start))
        .route("/api/auth/register/finish", post(register_finish))
}
```

- [ ] **Step 5: Compile**

```bash
cargo build
```

Expected: compiles. Fix any type errors by checking webauthn-rs 0.5 docs for exact type names.

- [ ] **Step 6: End-to-end manual test (local)**

```bash
cargo run
```

1. Register a passkey: `curl -X POST http://localhost:3000/api/auth/register/start` — should return a JSON challenge.
2. Open `http://localhost:3000/admin/login` in browser.
3. Since registration requires browser interaction with `navigator.credentials.create()`, open a temporary registration page. Create `templates/register.html` as a copy of `login.html` but with `startRegister()` wired to the button.
4. Register your passkey.
5. Visit `/admin/login`, log in with passkey, get redirected to `/admin`.
6. Upload a post (requires real R2 credentials — skip for now if testing locally).
7. Log out — redirected to `/admin/login`.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat: WebAuthn passkey authentication (register + login)"
```

---

## Phase 8: Deployment

### Task 8: Hetzner VPS setup and deployment

**Files:**
- Create: `deploy/portfolio.service` (systemd service)
- Create: `deploy/nginx.conf`

- [ ] **Step 1: Create systemd service file**

Create `deploy/portfolio.service`:
```ini
[Unit]
Description=Drawing Portfolio
After=network.target

[Service]
Type=simple
User=portfolio
WorkingDirectory=/opt/portfolio
EnvironmentFile=/opt/portfolio/.env
ExecStart=/opt/portfolio/drawingportfolio
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
```

- [ ] **Step 2: Create nginx config**

Create `deploy/nginx.conf`:
```nginx
# Rate limiting for auth endpoints
limit_req_zone $binary_remote_addr zone=auth:10m rate=10r/m;

server {
    listen 80;
    server_name yourdomain.com;
    return 301 https://$host$request_uri;
}

server {
    listen 443 ssl;
    server_name yourdomain.com;

    ssl_certificate /etc/letsencrypt/live/yourdomain.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/yourdomain.com/privkey.pem;

    # Block external access to registration endpoints
    location /api/auth/register/ {
        deny all;
    }

    # Rate limit auth endpoints
    location /api/auth/ {
        limit_req zone=auth burst=5 nodelay;
        proxy_pass http://localhost:3000;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
    }

    location / {
        proxy_pass http://localhost:3000;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
    }
}
```

- [ ] **Step 3: Build release binary**

```bash
cargo build --release
```

Expected: `target/release/drawingportfolio` binary produced.

- [ ] **Step 4: Deploy to Hetzner VPS**

On VPS (run these commands after SSH-ing in):
```bash
# Create service user
sudo useradd -r -s /bin/false portfolio
sudo mkdir -p /opt/portfolio
sudo chown portfolio:portfolio /opt/portfolio
```

From local machine:
```bash
# Copy binary, static files, and templates
scp target/release/drawingportfolio user@your-vps:/opt/portfolio/
scp -r static templates user@your-vps:/opt/portfolio/
# Copy and fill in .env
scp .env.example user@your-vps:/opt/portfolio/.env
```

On VPS:
```bash
# Edit .env with real values
sudo nano /opt/portfolio/.env
sudo chmod 600 /opt/portfolio/.env
sudo chown portfolio:portfolio /opt/portfolio/.env
sudo chmod 600 /opt/portfolio/portfolio.db 2>/dev/null || true

# Install and start service
sudo cp deploy/portfolio.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable portfolio
sudo systemctl start portfolio
sudo systemctl status portfolio
```

- [ ] **Step 5: Set up nginx and HTTPS**

```bash
# On VPS
sudo apt install nginx certbot python3-certbot-nginx
sudo cp /path/to/deploy/nginx.conf /etc/nginx/sites-available/portfolio
sudo ln -s /etc/nginx/sites-available/portfolio /etc/nginx/sites-enabled/
# Edit nginx.conf to replace yourdomain.com with real domain
sudo nginx -t
sudo certbot --nginx -d yourdomain.com
sudo systemctl restart nginx
```

- [ ] **Step 6: Register passkey on VPS**

Since registration is localhost-only, you need a browser request that arrives at the VPS from localhost. **WebAuthn passkeys are bound to RP_ID/RP_ORIGIN** — a passkey registered against `localhost` will not work against `yourdomain.com`. So `.env` must already have the production values before you register.

The correct approach: create a temporary registration HTML page served by the running app, then use a browser on the VPS itself (e.g. Firefox in a desktop session, or a headless browser). If the VPS has no desktop, the simplest option is to temporarily allow registration from your IP in nginx, register via HTTPS on the real domain, then re-restrict:

```nginx
# Temporary: allow your IP for registration
location /api/auth/register/ {
    allow YOUR.IP.ADDRESS.HERE;
    deny all;
    proxy_pass http://localhost:3000;
}
```

1. Add your IP to nginx, reload: `sudo systemctl reload nginx`
2. Create a simple registration page (`templates/register.html`) — copy of `login.html` but calls `startRegister()` from `webauthn.js`
3. Visit `https://yourdomain.com/register-setup` and register your passkey from your real browser
4. Remove the temporary nginx allow rule and reload nginx
5. Delete the registration page and route

**`.env` must have production values throughout:**
```
RP_ID=yourdomain.com
RP_ORIGIN=https://yourdomain.com
```

- [ ] **Step 7: Smoke test production**

1. Visit `https://yourdomain.com` — feed loads
2. Visit `https://yourdomain.com/admin/login` — login page loads
3. Log in with passkey
4. Upload a drawing — appears in feed
5. Check R2 bucket in Cloudflare dashboard — image appears

- [ ] **Step 8: Final commit**

```bash
git add deploy/
git commit -m "feat: deployment config (systemd + nginx)"
```

---

## Reference

**Running tests:**
```bash
cargo test              # all tests
cargo test db::tests    # DB layer only
```

**Local dev loop:**
```bash
cargo run               # starts on localhost:3000
```

**Redeploy after code change:**
```bash
cargo build --release
scp target/release/drawingportfolio user@vps:/opt/portfolio/
ssh user@vps "sudo systemctl restart portfolio"
```

**webauthn-rs 0.5 docs:** https://docs.rs/webauthn-rs/0.5/webauthn_rs/

**Cloudflare R2 + aws-sdk-rust:** https://developers.cloudflare.com/r2/examples/aws/aws-sdk-rust/
