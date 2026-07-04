# Logging and Tests Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **Branch:** Work on `feature/logging-and-tests` branch, merge to `master` when complete.

**Goal:** Add structured request/event logging throughout the Axum app and fill gaps in the test suite so production issues are diagnosable via `journalctl -u portfolio -f`.

**Architecture:** Use `tower_http::TraceLayer` for per-request logging (already a dependency via `ServeDir`), and `tracing::info!/warn!/error!` macros for discrete events. Tests use in-memory SQLite (already established pattern in `src/db.rs`).

**Tech Stack:** Rust · Axum 0.8 · tower-http 0.6 `TraceLayer` · `tracing` crate · sqlx in-memory SQLite for tests · tokio::test

---

## Files Modified

- `src/main.rs` — add `TraceLayer` to router
- `src/middleware.rs` — log rejected session attempts
- `src/routes/auth.rs` — log login/register/logout events
- `src/routes/admin.rs` — log uploads, deletes, and add missing tests
- `src/db.rs` — log cleanup rows affected, add missing tests
- `src/routes/feed.rs` — add missing pagination and HTMX tests

---

### Task 1: Per-request tracing via TraceLayer

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Write a test to verify the server starts (already exists — just ensure build passes after change)**

Run: `cargo test test_api_posts_empty 2>&1`
Expected: PASS (baseline before changes)

- [ ] **Step 2: Add TraceLayer to the router in `src/main.rs`**

Add import at top of file (tower_http is already in Cargo.toml):
```rust
use tower_http::trace::TraceLayer;
```

Add `.layer(TraceLayer::new_for_http())` to the router chain:
```rust
let app = Router::new()
    .merge(routes::hub::router())
    .merge(routes::feed::router())
    .merge(routes::admin::router())
    .merge(routes::auth::router())
    .nest_service("/static", ServeDir::new("static"))
    .layer(TraceLayer::new_for_http())
    .with_state(state)
    .fallback(handler_404);
```

- [ ] **Step 3: Build to verify it compiles**

Run: `cargo build 2>&1`
Expected: no errors (warnings OK)

- [ ] **Step 4: Commit**

```bash
git add src/main.rs
git commit -m "feat: add per-request tracing via TraceLayer"
```

---

### Task 2: Log rejected session attempts in middleware

**Files:**
- Modify: `src/middleware.rs`

- [ ] **Step 1: Add tracing to the AuthSession rejection path**

The `from_request_parts` function in `src/middleware.rs` currently redirects silently. Add a warn log before the redirect:

```rust
impl FromRequestParts<Arc<AppState>> for AuthSession {
    type Rejection = axum::response::Response;

    async fn from_request_parts(parts: &mut Parts, state: &Arc<AppState>) -> Result<Self, Self::Rejection> {
        let session_id = extract_session_cookie(parts);

        if let Some(id) = session_id {
            if db::get_session(&state.pool, &id).await.is_some() {
                return Ok(AuthSession(id));
            }
            tracing::warn!("rejected expired/invalid session");
        } else {
            tracing::warn!("rejected request with no session cookie");
        }

        Err(Redirect::to("/admin/login").into_response())
    }
}
```

- [ ] **Step 2: Build to verify**

Run: `cargo build 2>&1`
Expected: no errors

- [ ] **Step 3: Commit**

```bash
git add src/middleware.rs
git commit -m "feat: log rejected session attempts"
```

---

### Task 3: Log auth events (login, register, logout)

**Files:**
- Modify: `src/routes/auth.rs`

- [ ] **Step 1: Add login success/failure logging in `login_finish`**

Find the `login_finish` function in `src/routes/auth.rs`. In the `Ok(_result)` branch, add after creating the session:
```rust
tracing::info!("login successful, session created");
```

In the `Err(e)` branch at the end of `finish_passkey_authentication`:
```rust
Err(e) => {
    tracing::warn!("login failed: {e}");
    Json(serde_json::json!({ "ok": false, "error": e.to_string() })).into_response()
}
```

- [ ] **Step 2: Add login_start warning when no passkeys registered**

In `login_start`, the early return for empty passkeys:
```rust
if passkeys.is_empty() {
    tracing::warn!("login attempted but no passkeys registered");
    return (StatusCode::FORBIDDEN, ...
```

- [ ] **Step 3: Add register success logging in `register_finish`**

In the `Ok(passkey)` branch of `register_finish`:
```rust
Ok(passkey) => {
    // ... existing credential save code ...
    tracing::info!("passkey registered successfully, cred_id={cred_id}");
    Json(serde_json::json!({ "ok": true })).into_response()
}
```

- [ ] **Step 4: Add logout logging in `logout`**

At the start of the `logout` function, after the cookie loop finds a session:
```rust
async fn logout(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if let Some(cookies) = headers.get("cookie").and_then(|v| v.to_str().ok()) {
        for cookie in cookies.split(';') {
            if let Some(id) = cookie.trim().strip_prefix("session=") {
                tracing::info!("logout: deleting session");
                db::delete_session(&state.pool, id).await;
            }
        }
    }
    // ... rest unchanged
```

- [ ] **Step 5: Build to verify**

Run: `cargo build 2>&1`
Expected: no errors

- [ ] **Step 6: Commit**

```bash
git add src/routes/auth.rs
git commit -m "feat: log auth events (login, register, logout)"
```

---

### Task 4: Log upload and delete events in admin

**Files:**
- Modify: `src/routes/admin.rs`

- [ ] **Step 1: Add upload logging in `upload_post`**

After the magic bytes check succeeds (when `image_data` is set), the upload call already has an error log. Add a success log after `insert_post`:

```rust
let post = crate::db::insert_post(&state.pool, caption.trim(), &image_url).await;
tracing::info!("post created: id={}, key={key}, size={} bytes", post.id, bytes_len);
Html(admin_post_card_html(&post)).into_response()
```

To get `bytes_len`, save it before moving `bytes` into the upload call:
```rust
let bytes_len = bytes.len();
let image_url = match state.storage.upload(&key, bytes, &content_type).await {
```

- [ ] **Step 2: Add delete logging in `delete_post`**

```rust
async fn delete_post(
    _session: crate::middleware::AuthSession,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    if let Some(image_url) = crate::db::delete_post_and_get_url(&state.pool, id).await {
        tracing::info!("deleting post id={id}");
        if let Err(e) = state.storage.delete_by_url(&image_url).await {
            tracing::error!("storage delete failed for post id={id}: {e}");
        }
    } else {
        tracing::warn!("delete requested for nonexistent post id={id}");
    }
    StatusCode::OK
}
```

- [ ] **Step 3: Build to verify**

Run: `cargo build 2>&1`
Expected: no errors

- [ ] **Step 4: Commit**

```bash
git add src/routes/admin.rs
git commit -m "feat: log upload and delete events in admin"
```

---

### Task 5: Log cleanup rows affected in db.rs

**Files:**
- Modify: `src/db.rs`

- [ ] **Step 1: Update `cleanup_expired` to log rows deleted**

The current function discards the result with `.ok()`. Change it to capture and log:

```rust
pub async fn cleanup_expired(pool: &DbPool) {
    let sessions = sqlx::query!("DELETE FROM sessions WHERE expires_at <= datetime('now')")
        .execute(pool)
        .await
        .ok();
    let challenges = sqlx::query!("DELETE FROM auth_challenge_state WHERE expires_at <= datetime('now')")
        .execute(pool)
        .await
        .ok();

    let session_rows = sessions.map(|r| r.rows_affected()).unwrap_or(0);
    let challenge_rows = challenges.map(|r| r.rows_affected()).unwrap_or(0);
    tracing::info!("cleanup: removed {session_rows} expired sessions, {challenge_rows} expired challenges");
}
```

- [ ] **Step 2: Build to verify**

Run: `cargo build 2>&1`
Expected: no errors

- [ ] **Step 3: Commit**

```bash
git add src/db.rs
git commit -m "feat: log rows affected in cleanup_expired"
```

---

### Task 6: Add missing db.rs tests

**Files:**
- Modify: `src/db.rs` — add 3 tests inside existing `#[cfg(test)]` block

- [ ] **Step 1: Write the three failing tests**

Add inside the `mod tests` block in `src/db.rs` (after the existing `test_session_lifecycle` test):

```rust
#[tokio::test]
async fn test_expired_session_rejected() {
    let pool = test_pool().await;
    create_session(&pool, "expired-id", "2000-01-01T00:00:00").await;
    assert!(get_session(&pool, "expired-id").await.is_none());
}

#[tokio::test]
async fn test_cleanup_removes_expired() {
    let pool = test_pool().await;
    create_session(&pool, "old-session", "2000-01-01T00:00:00").await;
    save_challenge(&pool, "old-challenge", "{}", "2000-01-01T00:00:00").await;
    cleanup_expired(&pool).await;
    assert!(get_session(&pool, "old-session").await.is_none());
    assert!(take_challenge(&pool, "old-challenge").await.is_none());
}

#[tokio::test]
async fn test_delete_nonexistent_post_returns_none() {
    let pool = test_pool().await;
    let result = delete_post_and_get_url(&pool, 99999).await;
    assert!(result.is_none());
}
```

- [ ] **Step 2: Run tests to verify they fail (or pass — expired session may already pass)**

Run: `cargo test --lib db 2>&1`
Expected: all 3 new tests compile and either PASS or FAIL with a clear reason (not a compile error)

- [ ] **Step 3: Run full test suite to verify nothing broken**

Run: `cargo test 2>&1`
Expected: all tests pass

- [ ] **Step 4: Commit**

```bash
git add src/db.rs
git commit -m "test: add expired session, cleanup, and missing post tests"
```

---

### Task 7: Add missing admin.rs tests

**Files:**
- Modify: `src/routes/admin.rs` — add 2 tests inside existing `#[cfg(test)]` block

- [ ] **Step 1: Write the two failing tests**

Add inside the `mod tests` block in `src/routes/admin.rs` (after existing magic bytes tests):

```rust
#[test]
fn test_magic_bytes_webp() {
    let mut webp = b"RIFF".to_vec();
    webp.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]); // file size (ignored)
    webp.extend_from_slice(b"WEBP");
    assert_eq!(validate_magic_bytes(&webp), Some("webp"));
}

#[test]
fn test_html_escape_special_chars() {
    assert_eq!(html_escape("a & b"), "a &amp; b");
    assert_eq!(html_escape("<script>"), "&lt;script&gt;");
    assert_eq!(html_escape("\"quoted\""), "&quot;quoted&quot;");
}
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test --lib routes::admin 2>&1`
Expected: both new tests PASS

- [ ] **Step 3: Commit**

```bash
git add src/routes/admin.rs
git commit -m "test: add webp magic bytes and html_escape tests"
```

---

### Task 8: Add missing feed.rs tests

**Files:**
- Modify: `src/routes/feed.rs` — add 2 tests inside existing `#[cfg(test)]` block

- [ ] **Step 1: Write the two tests**

Add inside the `mod tests` block in `src/routes/feed.rs` (after existing `test_api_posts_empty`):

```rust
#[tokio::test]
async fn test_api_posts_has_more() {
    let app = test_app().await;
    // Insert 21 posts via the pool directly
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .unwrap();
    // We can't easily insert into the test_app's pool from outside,
    // so test has_more logic via the db function directly
    let pool = {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .unwrap();
        crate::db::run_migrations(&pool).await;
        for i in 0..21 {
            crate::db::insert_post(&pool, &format!("caption {i}"), "https://example.com/img.jpg").await;
        }
        pool
    };
    let mut posts = crate::db::get_posts(&pool, 0).await;
    let has_more = posts.len() > 20;
    assert!(has_more, "expected has_more=true with 21 posts");
    posts.truncate(20);
    assert_eq!(posts.len(), 20);
}

#[test]
fn test_post_card_html_escapes_content() {
    let post = crate::models::Post {
        id: 1,
        caption: "<script>alert(1)</script>".to_string(),
        image_url: "https://example.com/img.jpg".to_string(),
        created_at: "2024-01-01T00:00:00".to_string(),
    };
    let html = post_card_html(&post);
    assert!(!html.contains("<script>"), "raw script tag should be escaped");
    assert!(html.contains("&lt;script&gt;"));
}
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test --lib routes::feed 2>&1`
Expected: all 3 feed tests PASS

- [ ] **Step 3: Run full test suite**

Run: `cargo test 2>&1`
Expected: all tests pass, count should now be 10+ tests

- [ ] **Step 4: Commit**

```bash
git add src/routes/feed.rs
git commit -m "test: add pagination has_more and post card XSS escaping tests"
```

---

### Task 9: Final verification

- [ ] **Step 1: Run full test suite**

Run: `cargo test 2>&1`
Expected: all tests pass

- [ ] **Step 2: Run clippy**

Run: `cargo clippy 2>&1`
Expected: no new errors (pre-existing dead_code warnings OK)

- [ ] **Step 3: Verify log output looks correct locally**

Run: `cargo run 2>&1 &` then `curl http://localhost:3000/` and `curl http://localhost:3000/artportfolio`

Expected output in terminal should include lines like:
```
INFO drawingportfolio: listening on 0.0.0.0:3000
INFO request{method=GET uri=/ ...}: tower_http::trace: finished processing request status=200
```

- [ ] **Step 4: Merge and push to trigger deploy**

```bash
git checkout master
git merge feature/logging-and-tests
git push
```

Then on server verify: `journalctl -u portfolio -f` shows request logs on page load.
