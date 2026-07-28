# Drinking Game v1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship the Jackbox-style drink tracker (spec: `docs/superpowers/specs/2026-07-28-drinking-game-v1-design.md`) as a new `drinkinggame` workspace crate mounted at `/drinks` in the portfolio server.

**Architecture:** A lib crate exposing `pub async fn router(Config) -> axum::Router` that owns its own SQLite pool, DB-backed sessions (name + argon2-hashed 4-digit PIN), and a per-room `tokio::sync::broadcast` hub. Client → server is HTMX form posts; server → client is SSE carrying server-rendered leaderboard fragments swapped in by ~5 lines of native `EventSource` JS. The portfolio binary nests it via `.nest_service("/drinks", ...)`; a thin bin target serves it standalone on :3001 for local testing.

**Tech Stack:** Rust, Axum 0.8, sqlx 0.8 (SQLite, **runtime-checked queries — no `query!` macros, no `.sqlx` cache for this crate**), Askama 0.15 (manual `.render()` + `Html`), argon2, tokio broadcast + SSE, HTMX (embedded copy of the repo's `static/htmx.min.js`).

## Global Constraints

- Versions track the portfolio: `axum = "0.8"`, `sqlx = "0.8"`, `askama = "0.15"`, `tower = "0.5"`. The game crate does NOT depend on chrono (all time math happens in SQLite via `datetime('now', ...)`), so the workspace-wide `chrono =0.4.34` pin is unaffected.
- The game crate must never depend on the portfolio crate or its `AppState`. Integration is one `.nest_service()` call.
- All SQL lives in `drinkinggame/src/db.rs` (mirrors the portfolio's db.rs rule). Handlers call db functions only.
- Templates receive pre-computed values; no logic beyond simple conditionals.
- Timestamps are ISO8601 `TEXT` via SQLite `datetime('now')` — never `DATETIME` columns, never UNIX integers (matches portfolio convention; avoids sqlx nullable inference issues).
- All URLs in templates are built from `base_path` (`""` standalone, `"/drinks"` nested). Never relative URLs — from `/drinks/room/ABCD` a relative `sse` resolves to `/drinks/room/sse` and breaks silently.
- `SQLX_OFFLINE=true cargo build --release` at the workspace root must keep working after every task (the game crate has no compile-time-checked queries, so this is automatic — but verify in Task 1 and Task 12).
- Commit style: conventional commits (`feat:`, `fix:`, `chore:`, `docs:`), matching repo history.
- The deploy artifact stays `target/release/drawingportfolio` — deploy.yml must need zero changes.

---

### Task 1: Cargo workspace conversion + crate skeleton

**Files:**
- Modify: `Cargo.toml` (repo root)
- Create: `drinkinggame/Cargo.toml`
- Create: `drinkinggame/src/lib.rs`
- Modify: `.gitignore`

**Interfaces:**
- Produces: workspace member `drinkinggame`; `drinkinggame::Config { database_url: String, base_path: String }`. Later tasks fill in the crate.

- [ ] **Step 1: Add the workspace section to the root Cargo.toml**

Append to the END of the root `Cargo.toml` (the root package automatically becomes a workspace member; do not add `"."` to members):

```toml
[workspace]
members = ["drinkinggame"]
```

- [ ] **Step 2: Create the crate manifest**

Create `drinkinggame/Cargo.toml`:

```toml
[package]
name = "drinkinggame"
version = "0.1.0"
edition = "2021"

[dependencies]
axum = "0.8"
tokio = { version = "1", features = ["full"] }
tokio-stream = { version = "0.1", features = ["sync"] }
futures = "0.3"
sqlx = { version = "0.8", features = ["sqlite", "runtime-tokio"] }
askama = "0.15"
# Manual .render() + axum::response::Html, matching the portfolio convention.
serde = { version = "1", features = ["derive"] }
argon2 = "0.5"
rand = "0.8"
thiserror = "2"
tracing = "0.1"
# For the standalone bin only:
dotenvy = "0.15"
tracing-subscriber = "0.3"

[dev-dependencies]
tower = { version = "0.5", features = ["util"] }
http-body-util = "0.1"
```

- [ ] **Step 3: Create the minimal lib.rs**

Create `drinkinggame/src/lib.rs`:

```rust
//! Drinking game — party drink tracker, mounted under /drinks in the
//! portfolio server or served standalone via the bin target.

/// Everything the crate needs from its host. No portfolio types leak in here.
pub struct Config {
    /// e.g. "sqlite:./drinkinggame.db"
    pub database_url: String,
    /// URL prefix the router is mounted under: "" standalone, "/drinks" nested.
    /// Used only for URL generation in templates/redirects — routing itself
    /// is prefix-agnostic because .nest_service() strips the prefix.
    pub base_path: String,
}
```

- [ ] **Step 4: Ignore the game's dev database**

Add to `.gitignore`:

```
drinkinggame.db*
```

(The `*` also catches SQLite's `-wal`/`-shm` sidecar files.)

- [ ] **Step 5: Verify the workspace builds and the deploy artifact path is unchanged**

Run: `cargo build && cargo test && SQLX_OFFLINE=true cargo build --release && ls target/release/drawingportfolio`
Expected: all green; `target/release/drawingportfolio` exists (workspace shares one `target/`, so deploy.yml needs no change). `cargo test` runs the existing 35 portfolio tests.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock drinkinggame/ .gitignore
git commit -m "chore: convert repo to cargo workspace, add empty drinkinggame crate"
```

---

### Task 2: Schema, models, error type, db foundation

**Files:**
- Create: `drinkinggame/migrations/001_initial.sql`
- Create: `drinkinggame/src/db.rs`
- Create: `drinkinggame/src/models.rs`
- Create: `drinkinggame/src/error.rs`
- Create: `drinkinggame/src/render.rs`
- Modify: `drinkinggame/src/lib.rs` (module declarations)
- Test: inline `#[cfg(test)]` in `drinkinggame/src/db.rs`

**Interfaces:**
- Produces:
  - `db::DbPool` (= `sqlx::SqlitePool`), `db::connect(url: &str) -> DbPool`, `db::run_migrations(pool: &DbPool)`
  - `db::get_player_by_name(pool, name: &str) -> Option<Player>`, `db::insert_player(pool, name: &str, pin_hash: &str) -> Result<i64, sqlx::Error>`
  - `models::Player { id: i64, name: String, pin_hash: String, created_at: String }`
  - `models::Room { id: i64, code: String, created_at: String, last_activity_at: String, ended_at: Option<String> }`
  - `models::LeaderboardRow { name: String, drinks: i64, shots: i64 }`
  - `error::GameError` (`InvalidName | InvalidPin | WrongPin | RoomNotFound | Db(sqlx::Error)`), implements `IntoResponse` as an HTML fragment
  - `render::html_escape(s: &str) -> String`

- [ ] **Step 1: Write the migration**

Create `drinkinggame/migrations/001_initial.sql`:

```sql
-- Drinking game v1. All timestamps are ISO8601 TEXT (portfolio convention).
CREATE TABLE IF NOT EXISTS players (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE COLLATE NOCASE,
    pin_hash TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS sessions (
    id TEXT PRIMARY KEY,
    player_id INTEGER NOT NULL REFERENCES players(id),
    expires_at TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS rooms (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    code TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    last_activity_at TEXT NOT NULL DEFAULT (datetime('now')),
    ended_at TEXT
);

-- Join codes are unique only among OPEN rooms; ended rooms free their code.
CREATE UNIQUE INDEX IF NOT EXISTS idx_rooms_open_code
    ON rooms(code) WHERE ended_at IS NULL;

CREATE TABLE IF NOT EXISTS room_players (
    room_id INTEGER NOT NULL REFERENCES rooms(id),
    player_id INTEGER NOT NULL REFERENCES players(id),
    joined_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (room_id, player_id)
);

-- Append-only. Undo is a tombstone (undone_at), never a DELETE.
CREATE TABLE IF NOT EXISTS events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    room_id INTEGER NOT NULL REFERENCES rooms(id),
    player_id INTEGER NOT NULL REFERENCES players(id),
    kind TEXT NOT NULL CHECK (kind IN ('drink', 'shot')),
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    undone_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_events_room ON events(room_id, player_id);
```

- [ ] **Step 2: Write models.rs**

Create `drinkinggame/src/models.rs`:

```rust
//! Plain data structs mirroring database rows. FromRow lets sqlx's
//! runtime-checked query_as map columns by name.

#[derive(sqlx::FromRow, Clone, Debug)]
pub struct Player {
    pub id: i64,
    pub name: String,
    pub pin_hash: String,
    pub created_at: String,
}

#[derive(sqlx::FromRow, Clone, Debug)]
pub struct Room {
    pub id: i64,
    pub code: String,
    pub created_at: String,
    pub last_activity_at: String,
    pub ended_at: Option<String>,
}

#[derive(sqlx::FromRow, Clone, Debug, PartialEq)]
pub struct LeaderboardRow {
    pub name: String,
    pub drinks: i64,
    pub shots: i64,
}
```

- [ ] **Step 3: Write error.rs**

Create `drinkinggame/src/error.rs`:

```rust
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};

/// Typed domain errors. IntoResponse renders an HTML fragment suitable for
/// an HTMX swap; full-page errors (e.g. visiting a dead room URL) are
/// rendered by the route handlers themselves, which know base_path.
#[derive(thiserror::Error, Debug)]
pub enum GameError {
    #[error("name must be 1\u{2013}20 characters")]
    InvalidName,
    #[error("PIN must be exactly 4 digits")]
    InvalidPin,
    #[error("wrong PIN for that name")]
    WrongPin,
    #[error("room not found or already ended")]
    RoomNotFound,
    #[error("something went wrong, try again")]
    Db(#[from] sqlx::Error),
}

impl IntoResponse for GameError {
    fn into_response(self) -> Response {
        let status = match &self {
            GameError::InvalidName | GameError::InvalidPin => StatusCode::UNPROCESSABLE_ENTITY,
            GameError::WrongPin => StatusCode::UNAUTHORIZED,
            GameError::RoomNotFound => StatusCode::NOT_FOUND,
            GameError::Db(e) => {
                tracing::error!("db error: {e}");
                StatusCode::INTERNAL_SERVER_ERROR
            }
        };
        let body = format!(
            r#"<p class="error">{}</p>"#,
            crate::render::html_escape(&self.to_string())
        );
        (status, [(header::CONTENT_TYPE, "text/html; charset=utf-8")], body).into_response()
    }
}
```

- [ ] **Step 4: Write render.rs (escape only for now)**

Create `drinkinggame/src/render.rs`:

```rust
//! HTML fragment builders (format! strings, matching the portfolio's
//! post_card_html convention) plus escaping.

/// Same escape set as the portfolio's html_escape.
pub fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
```

- [ ] **Step 5: Write the failing db tests**

Create `drinkinggame/src/db.rs` with the test module first:

```rust
use crate::models::Player;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::str::FromStr;

pub type DbPool = sqlx::SqlitePool;

pub async fn connect(url: &str) -> DbPool {
    let opts = SqliteConnectOptions::from_str(url)
        .expect("invalid drinks DATABASE_URL")
        .create_if_missing(true);
    SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(opts)
        .await
        .expect("failed to connect to drinks db")
}

pub async fn run_migrations(pool: &DbPool) {
    sqlx::query(include_str!("../migrations/001_initial.sql"))
        .execute(pool)
        .await
        .expect("drinks migration 001 failed");
}

pub async fn get_player_by_name(pool: &DbPool, name: &str) -> Option<Player> {
    sqlx::query_as::<_, Player>(
        "SELECT id, name, pin_hash, created_at FROM players WHERE name = ?1",
    )
    .bind(name)
    .fetch_optional(pool)
    .await
    .expect("get_player_by_name failed")
}

/// Returns Err on UNIQUE violation (name taken) — callers handle the race.
pub async fn insert_player(pool: &DbPool, name: &str, pin_hash: &str) -> Result<i64, sqlx::Error> {
    let res = sqlx::query("INSERT INTO players (name, pin_hash) VALUES (?1, ?2)")
        .bind(name)
        .bind(pin_hash)
        .execute(pool)
        .await?;
    Ok(res.last_insert_rowid())
}

#[cfg(test)]
pub(crate) async fn test_pool() -> DbPool {
    // max_connections(1): each :memory: connection is a SEPARATE empty db,
    // so the pool must never open a second one.
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    run_migrations(&pool).await;
    pool
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_migrations_are_idempotent() {
        let pool = test_pool().await;
        run_migrations(&pool).await; // second run must not error
    }

    #[tokio::test]
    async fn test_insert_and_get_player() {
        let pool = test_pool().await;
        let id = insert_player(&pool, "hampter", "fakehash").await.unwrap();
        let p = get_player_by_name(&pool, "hampter").await.unwrap();
        assert_eq!(p.id, id);
        assert_eq!(p.pin_hash, "fakehash");
        assert!(!p.created_at.is_empty());
    }

    #[tokio::test]
    async fn test_player_name_is_case_insensitive_unique() {
        let pool = test_pool().await;
        insert_player(&pool, "Hampter", "h1").await.unwrap();
        assert!(insert_player(&pool, "hampter", "h2").await.is_err());
        // Lookup also matches case-insensitively (COLLATE NOCASE on the column).
        assert!(get_player_by_name(&pool, "HAMPTER").await.is_some());
    }
}
```

- [ ] **Step 6: Wire modules into lib.rs**

In `drinkinggame/src/lib.rs`, add above `pub struct Config`:

```rust
pub mod db;
pub mod error;
pub mod models;
pub mod render;
```

- [ ] **Step 7: Run the tests**

Run: `cargo test -p drinkinggame`
Expected: 3 tests PASS.

- [ ] **Step 8: Commit**

```bash
git add drinkinggame/
git commit -m "feat(drinks): schema, models, error type, db foundation"
```

---

### Task 3: Identity — PIN hashing, validation, login-or-register, sessions

**Files:**
- Create: `drinkinggame/src/auth.rs` (pure logic half — the extractors come in Task 7)
- Modify: `drinkinggame/src/db.rs` (session functions)
- Modify: `drinkinggame/src/lib.rs` (`pub mod auth;`)
- Test: inline in both files

**Interfaces:**
- Consumes: `db::get_player_by_name`, `db::insert_player`, `GameError`
- Produces:
  - `auth::COOKIE_NAME: &str = "dg_session"`
  - `auth::validate_name(&str) -> Result<String, GameError>` (returns trimmed owned name)
  - `auth::validate_pin(&str) -> Result<(), GameError>`
  - `auth::hash_pin(&str) -> String`, `auth::verify_pin(pin: &str, hash: &str) -> bool` (sync; callers wrap in spawn_blocking)
  - `auth::login_or_register(pool: &DbPool, name: &str, pin: &str) -> Result<Player, GameError>`
  - `auth::new_session_id() -> String` (64 hex chars)
  - `auth::session_cookie(id: &str) -> String`
  - `db::create_session(pool, id: &str, player_id: i64, ttl: &str)` — ttl is a SQLite modifier like `"+90 days"`
  - `db::get_session_player(pool, session_id: &str) -> Option<Player>`
  - `db::cleanup_expired_sessions(pool)`

- [ ] **Step 1: Add session functions + failing tests to db.rs**

Append to `drinkinggame/src/db.rs`:

```rust
/// ttl is a SQLite datetime modifier, e.g. "+90 days". Tests pass "-1 days"
/// to create an already-expired session.
pub async fn create_session(pool: &DbPool, id: &str, player_id: i64, ttl: &str) {
    sqlx::query(
        "INSERT INTO sessions (id, player_id, expires_at) VALUES (?1, ?2, datetime('now', ?3))",
    )
    .bind(id)
    .bind(player_id)
    .bind(ttl)
    .execute(pool)
    .await
    .expect("create_session failed");
}

pub async fn get_session_player(pool: &DbPool, session_id: &str) -> Option<Player> {
    sqlx::query_as::<_, Player>(
        "SELECT p.id, p.name, p.pin_hash, p.created_at
         FROM sessions s JOIN players p ON p.id = s.player_id
         WHERE s.id = ?1 AND s.expires_at > datetime('now')",
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await
    .expect("get_session_player failed")
}

pub async fn cleanup_expired_sessions(pool: &DbPool) {
    sqlx::query("DELETE FROM sessions WHERE expires_at <= datetime('now')")
        .execute(pool)
        .await
        .expect("cleanup_expired_sessions failed");
}
```

And in the `tests` module:

```rust
    #[tokio::test]
    async fn test_session_roundtrip_and_expiry() {
        let pool = test_pool().await;
        let pid = insert_player(&pool, "sess", "h").await.unwrap();
        create_session(&pool, "tok-live", pid, "+90 days").await;
        create_session(&pool, "tok-dead", pid, "-1 days").await;

        assert_eq!(get_session_player(&pool, "tok-live").await.unwrap().id, pid);
        assert!(get_session_player(&pool, "tok-dead").await.is_none());
        assert!(get_session_player(&pool, "tok-unknown").await.is_none());

        cleanup_expired_sessions(&pool).await;
        // Live session survives the sweep.
        assert!(get_session_player(&pool, "tok-live").await.is_some());
    }
```

- [ ] **Step 2: Run to verify the new test fails to compile, then passes once Step 1's impl is in**

Run: `cargo test -p drinkinggame test_session_roundtrip_and_expiry`
Expected: PASS (write test first, watch it fail against a stub if you prefer strict TDD ordering; the deliverable gate is a passing suite with the test present).

- [ ] **Step 3: Write auth.rs**

Create `drinkinggame/src/auth.rs`:

```rust
//! Name + 4-digit-PIN identity. Argon2 hashing is CPU-bound (~tens of ms on
//! the cx23), so async callers run hash/verify inside spawn_blocking.

use argon2::password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use rand::RngCore;

use crate::db::{self, DbPool};
use crate::error::GameError;
use crate::models::Player;

pub const COOKIE_NAME: &str = "dg_session";

pub fn validate_name(name: &str) -> Result<String, GameError> {
    let trimmed = name.trim();
    if trimmed.is_empty() || trimmed.chars().count() > 20 {
        return Err(GameError::InvalidName);
    }
    Ok(trimmed.to_string())
}

pub fn validate_pin(pin: &str) -> Result<(), GameError> {
    if pin.len() == 4 && pin.bytes().all(|b| b.is_ascii_digit()) {
        Ok(())
    } else {
        Err(GameError::InvalidPin)
    }
}

pub fn hash_pin(pin: &str) -> String {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(pin.as_bytes(), &salt)
        .expect("argon2 hashing failed")
        .to_string()
}

pub fn verify_pin(pin: &str, hash: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(hash) else { return false };
    Argon2::default().verify_password(pin.as_bytes(), &parsed).is_ok()
}

/// Known name -> verify PIN. Unknown name -> register. A lost create race
/// (two phones registering the same name simultaneously) falls back to verify.
pub async fn login_or_register(pool: &DbPool, name: &str, pin: &str) -> Result<Player, GameError> {
    let name = validate_name(name)?;
    validate_pin(pin)?;

    if let Some(player) = db::get_player_by_name(pool, &name).await {
        return check_pin(player, pin).await;
    }

    let pin_owned = pin.to_string();
    let hash = tokio::task::spawn_blocking(move || hash_pin(&pin_owned))
        .await
        .expect("hash task panicked");

    match db::insert_player(pool, &name, &hash).await {
        Ok(_) => Ok(db::get_player_by_name(pool, &name)
            .await
            .expect("player row must exist after insert")),
        // UNIQUE violation: someone else registered this name between our
        // lookup and insert. Treat it as a login attempt against their PIN.
        Err(_) => {
            let player = db::get_player_by_name(pool, &name)
                .await
                .ok_or(GameError::WrongPin)?;
            check_pin(player, pin).await
        }
    }
}

async fn check_pin(player: Player, pin: &str) -> Result<Player, GameError> {
    let pin = pin.to_string();
    let hash = player.pin_hash.clone();
    let ok = tokio::task::spawn_blocking(move || verify_pin(&pin, &hash))
        .await
        .unwrap_or(false);
    if ok { Ok(player) } else { Err(GameError::WrongPin) }
}

/// 32 random bytes as 64 hex chars — same entropy class as the portfolio's
/// session IDs.
pub fn new_session_id() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// SameSite=Lax (not Strict): the join flow follows links shared between
/// phones, and Strict would drop the cookie on those navigations.
pub fn session_cookie(id: &str) -> String {
    format!("{COOKIE_NAME}={id}; HttpOnly; SameSite=Lax; Max-Age=7776000; Path=/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pin_hash_roundtrip() {
        let hash = hash_pin("1234");
        assert!(verify_pin("1234", &hash));
        assert!(!verify_pin("4321", &hash));
        assert!(!verify_pin("1234", "not-a-hash"));
    }

    #[test]
    fn test_validation() {
        assert!(validate_pin("1234").is_ok());
        assert!(validate_pin("123").is_err());
        assert!(validate_pin("12345").is_err());
        assert!(validate_pin("12a4").is_err());
        assert_eq!(validate_name("  bob  ").unwrap(), "bob");
        assert!(validate_name("   ").is_err());
        assert!(validate_name(&"x".repeat(21)).is_err());
    }

    #[test]
    fn test_session_id_shape() {
        let a = new_session_id();
        let b = new_session_id();
        assert_eq!(a.len(), 64);
        assert_ne!(a, b);
        assert!(a.bytes().all(|c| c.is_ascii_hexdigit()));
    }

    #[tokio::test]
    async fn test_login_or_register_flow() {
        let pool = crate::db::test_pool().await;
        // First contact registers.
        let p1 = login_or_register(&pool, "alice", "1234").await.unwrap();
        // Same name + right PIN logs in as the same player.
        let p2 = login_or_register(&pool, "alice", "1234").await.unwrap();
        assert_eq!(p1.id, p2.id);
        // Wrong PIN is rejected.
        assert!(matches!(
            login_or_register(&pool, "alice", "9999").await,
            Err(GameError::WrongPin)
        ));
        // Bad inputs are rejected before touching the db.
        assert!(matches!(login_or_register(&pool, "", "1234").await, Err(GameError::InvalidName)));
        assert!(matches!(login_or_register(&pool, "bob", "12").await, Err(GameError::InvalidPin)));
    }
}
```

- [ ] **Step 4: Register the module**

In `drinkinggame/src/lib.rs` add `pub mod auth;` to the module list.

- [ ] **Step 5: Run all crate tests**

Run: `cargo test -p drinkinggame`
Expected: all PASS (argon2 tests take a few seconds — that's the KDF doing its job).

- [ ] **Step 6: Commit**

```bash
git add drinkinggame/
git commit -m "feat(drinks): name+PIN identity with argon2 and db sessions"
```

---

### Task 4: Rooms — code generation, create, join, activity, ending

**Files:**
- Create: `drinkinggame/src/rooms.rs`
- Modify: `drinkinggame/src/db.rs`
- Modify: `drinkinggame/src/lib.rs` (`pub mod rooms;`)
- Test: inline in both files

**Interfaces:**
- Consumes: `db::DbPool`, `models::Room`
- Produces:
  - `rooms::gen_room_code() -> String` (4 chars from `ABCDEFGHJKMNPQRSTUVWXYZ`)
  - `rooms::create_room_with_unique_code(pool) -> Room` (retry loop over the partial unique index)
  - `db::insert_room(pool, code: &str) -> Result<i64, sqlx::Error>`
  - `db::get_open_room(pool, code: &str) -> Option<Room>`
  - `db::get_room_by_id(pool, room_id: i64) -> Option<Room>`
  - `db::join_room(pool, room_id: i64, player_id: i64)` (idempotent, bumps activity)
  - `db::touch_room(pool, room_id: i64)`
  - `db::end_room(pool, room_id: i64)`
  - `db::end_inactive_rooms(pool, max_idle_hours: i64) -> Vec<i64>` (returns ended room ids)

- [ ] **Step 1: Add room functions to db.rs**

Append to `drinkinggame/src/db.rs` (add `use crate::models::Room;` to the imports):

```rust
pub async fn insert_room(pool: &DbPool, code: &str) -> Result<i64, sqlx::Error> {
    let res = sqlx::query("INSERT INTO rooms (code) VALUES (?1)")
        .bind(code)
        .execute(pool)
        .await?;
    Ok(res.last_insert_rowid())
}

pub async fn get_open_room(pool: &DbPool, code: &str) -> Option<Room> {
    sqlx::query_as::<_, Room>(
        "SELECT id, code, created_at, last_activity_at, ended_at
         FROM rooms WHERE code = ?1 AND ended_at IS NULL",
    )
    .bind(code)
    .fetch_optional(pool)
    .await
    .expect("get_open_room failed")
}

pub async fn get_room_by_id(pool: &DbPool, room_id: i64) -> Option<Room> {
    sqlx::query_as::<_, Room>(
        "SELECT id, code, created_at, last_activity_at, ended_at
         FROM rooms WHERE id = ?1",
    )
    .bind(room_id)
    .fetch_optional(pool)
    .await
    .expect("get_room_by_id failed")
}

/// Idempotent: rejoining is a no-op apart from the activity bump.
pub async fn join_room(pool: &DbPool, room_id: i64, player_id: i64) {
    sqlx::query("INSERT OR IGNORE INTO room_players (room_id, player_id) VALUES (?1, ?2)")
        .bind(room_id)
        .bind(player_id)
        .execute(pool)
        .await
        .expect("join_room failed");
    touch_room(pool, room_id).await;
}

pub async fn touch_room(pool: &DbPool, room_id: i64) {
    sqlx::query("UPDATE rooms SET last_activity_at = datetime('now') WHERE id = ?1")
        .bind(room_id)
        .execute(pool)
        .await
        .expect("touch_room failed");
}

pub async fn end_room(pool: &DbPool, room_id: i64) {
    sqlx::query("UPDATE rooms SET ended_at = datetime('now') WHERE id = ?1 AND ended_at IS NULL")
        .bind(room_id)
        .execute(pool)
        .await
        .expect("end_room failed");
}

/// Ends rooms idle longer than max_idle_hours; returns their ids so the
/// caller can drop broadcast channels.
pub async fn end_inactive_rooms(pool: &DbPool, max_idle_hours: i64) -> Vec<i64> {
    let modifier = format!("-{max_idle_hours} hours");
    let ids: Vec<(i64,)> = sqlx::query_as(
        "SELECT id FROM rooms
         WHERE ended_at IS NULL AND last_activity_at < datetime('now', ?1)",
    )
    .bind(&modifier)
    .fetch_all(pool)
    .await
    .expect("end_inactive_rooms select failed");

    sqlx::query(
        "UPDATE rooms SET ended_at = datetime('now')
         WHERE ended_at IS NULL AND last_activity_at < datetime('now', ?1)",
    )
    .bind(&modifier)
    .execute(pool)
    .await
    .expect("end_inactive_rooms update failed");

    ids.into_iter().map(|(id,)| id).collect()
}
```

- [ ] **Step 2: Write rooms.rs**

Create `drinkinggame/src/rooms.rs`:

```rust
//! Join-code generation. Codes are 4 letters from an alphabet with I, L and
//! O removed — they're ambiguous on a phone screen at 1am.

use rand::Rng;

use crate::db::{self, DbPool};
use crate::models::Room;

pub const CODE_ALPHABET: &[u8] = b"ABCDEFGHJKMNPQRSTUVWXYZ";
pub const CODE_LEN: usize = 4;

pub fn gen_room_code() -> String {
    let mut rng = rand::thread_rng();
    (0..CODE_LEN)
        .map(|_| CODE_ALPHABET[rng.gen_range(0..CODE_ALPHABET.len())] as char)
        .collect()
}

/// Insert with a fresh code, retrying on collision with an open room (the
/// partial unique index rejects duplicates among ended_at IS NULL rows).
/// 23^4 = 279,841 codes — collisions are rare, 20 retries is generous.
pub async fn create_room_with_unique_code(pool: &DbPool) -> Room {
    for _ in 0..20 {
        let code = gen_room_code();
        if let Ok(id) = db::insert_room(pool, &code).await {
            return db::get_room_by_id(pool, id)
                .await
                .expect("room row must exist after insert");
        }
    }
    panic!("could not find a free room code after 20 attempts");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_code_shape() {
        for _ in 0..100 {
            let code = gen_room_code();
            assert_eq!(code.len(), CODE_LEN);
            assert!(code.bytes().all(|b| CODE_ALPHABET.contains(&b)));
        }
    }

    #[tokio::test]
    async fn test_create_join_end_lifecycle() {
        let pool = crate::db::test_pool().await;
        let room = create_room_with_unique_code(&pool).await;
        assert!(db::get_open_room(&pool, &room.code).await.is_some());

        // Same code can't be inserted while the room is open...
        assert!(db::insert_room(&pool, &room.code).await.is_err());
        // ...but is freed once the room ends.
        db::end_room(&pool, room.id).await;
        assert!(db::get_open_room(&pool, &room.code).await.is_none());
        assert!(db::insert_room(&pool, &room.code).await.is_ok());
    }

    #[tokio::test]
    async fn test_join_is_idempotent() {
        let pool = crate::db::test_pool().await;
        let pid = db::insert_player(&pool, "j", "h").await.unwrap();
        let room = create_room_with_unique_code(&pool).await;
        db::join_room(&pool, room.id, pid).await;
        db::join_room(&pool, room.id, pid).await; // second join: no error, no dup
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM room_players WHERE room_id = ?1")
            .bind(room.id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count.0, 1);
    }

    #[tokio::test]
    async fn test_end_inactive_rooms() {
        let pool = crate::db::test_pool().await;
        let stale = create_room_with_unique_code(&pool).await;
        let fresh = create_room_with_unique_code(&pool).await;
        // Backdate the stale room 13 hours.
        sqlx::query("UPDATE rooms SET last_activity_at = datetime('now', '-13 hours') WHERE id = ?1")
            .bind(stale.id)
            .execute(&pool)
            .await
            .unwrap();

        let ended = db::end_inactive_rooms(&pool, 12).await;
        assert_eq!(ended, vec![stale.id]);
        assert!(db::get_open_room(&pool, &stale.code).await.is_none());
        assert!(db::get_open_room(&pool, &fresh.code).await.is_some());
    }
}
```

- [ ] **Step 3: Register the module**

In `drinkinggame/src/lib.rs` add `pub mod rooms;`.

- [ ] **Step 4: Run tests**

Run: `cargo test -p drinkinggame`
Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add drinkinggame/
git commit -m "feat(drinks): rooms with collision-safe join codes and inactivity ending"
```

---

### Task 5: Events — logging, tombstone undo, leaderboard fold

**Files:**
- Modify: `drinkinggame/src/db.rs`
- Modify: `drinkinggame/src/render.rs` (leaderboard fragment builder)
- Test: inline in both files

**Interfaces:**
- Consumes: `models::LeaderboardRow`, `render::html_escape`
- Produces:
  - `db::insert_event(pool, room_id: i64, player_id: i64, kind: &str)` (kind pre-validated by callers; CHECK constraint backstops)
  - `db::undo_last_event(pool, room_id: i64, player_id: i64) -> bool`
  - `db::leaderboard(pool, room_id: i64) -> Vec<LeaderboardRow>`
  - `db::lifetime_counts(pool, player_id: i64) -> (i64, i64)` (drinks, shots)
  - `render::leaderboard_items(rows: &[LeaderboardRow]) -> String` (concatenated `<li>` rows; the `<ol id="leaderboard">` container lives in templates)

- [ ] **Step 1: Add event functions to db.rs**

Append to `drinkinggame/src/db.rs` (add `use crate::models::LeaderboardRow;`):

```rust
pub async fn insert_event(pool: &DbPool, room_id: i64, player_id: i64, kind: &str) {
    sqlx::query("INSERT INTO events (room_id, player_id, kind) VALUES (?1, ?2, ?3)")
        .bind(room_id)
        .bind(player_id)
        .bind(kind)
        .execute(pool)
        .await
        .expect("insert_event failed");
}

/// Tombstones the caller's most recent live event in this room.
/// Returns false when there is nothing left to undo.
pub async fn undo_last_event(pool: &DbPool, room_id: i64, player_id: i64) -> bool {
    let res = sqlx::query(
        "UPDATE events SET undone_at = datetime('now')
         WHERE id = (
             SELECT id FROM events
             WHERE room_id = ?1 AND player_id = ?2 AND undone_at IS NULL
             ORDER BY id DESC LIMIT 1
         )",
    )
    .bind(room_id)
    .bind(player_id)
    .execute(pool)
    .await
    .expect("undo_last_event failed");
    res.rows_affected() > 0
}

/// Per-room standings: every member appears (LEFT JOIN), zero rows and all.
/// Sorted by total descending, then name for a stable order.
pub async fn leaderboard(pool: &DbPool, room_id: i64) -> Vec<LeaderboardRow> {
    sqlx::query_as::<_, LeaderboardRow>(
        "SELECT p.name,
                COALESCE(SUM(CASE WHEN e.kind = 'drink' THEN 1 ELSE 0 END), 0) AS drinks,
                COALESCE(SUM(CASE WHEN e.kind = 'shot'  THEN 1 ELSE 0 END), 0) AS shots
         FROM room_players rp
         JOIN players p ON p.id = rp.player_id
         LEFT JOIN events e
              ON e.room_id = rp.room_id
             AND e.player_id = rp.player_id
             AND e.undone_at IS NULL
         WHERE rp.room_id = ?1
         GROUP BY p.id
         ORDER BY (drinks + shots) DESC, p.name ASC",
    )
    .bind(room_id)
    .fetch_all(pool)
    .await
    .expect("leaderboard failed")
}

/// Lifetime totals across all rooms — the long-term profile stat.
pub async fn lifetime_counts(pool: &DbPool, player_id: i64) -> (i64, i64) {
    let row: (i64, i64) = sqlx::query_as(
        "SELECT COALESCE(SUM(CASE WHEN kind = 'drink' THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN kind = 'shot'  THEN 1 ELSE 0 END), 0)
         FROM events WHERE player_id = ?1 AND undone_at IS NULL",
    )
    .bind(player_id)
    .fetch_one(pool)
    .await
    .expect("lifetime_counts failed");
    row
}
```

- [ ] **Step 2: Add the fragment builder to render.rs**

Append to `drinkinggame/src/render.rs`:

```rust
use crate::models::LeaderboardRow;

/// Returns the <li> rows for the leaderboard. The surrounding
/// <ol id="leaderboard"> lives in the templates so SSE can replace
/// innerHTML wholesale.
pub fn leaderboard_items(rows: &[LeaderboardRow]) -> String {
    if rows.is_empty() {
        return r#"<li class="lb-empty">Nobody here yet</li>"#.to_string();
    }
    rows.iter()
        .map(|r| {
            format!(
                r#"<li><span class="lb-name">{}</span><span class="lb-counts">{} drinks &middot; {} shots</span></li>"#,
                html_escape(&r.name),
                r.drinks,
                r.shots
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape() {
        assert_eq!(html_escape("<b>&\"'"), "&lt;b&gt;&amp;&quot;&#39;");
    }

    #[test]
    fn test_leaderboard_items_escapes_names() {
        let rows = vec![LeaderboardRow { name: "<script>".into(), drinks: 2, shots: 1 }];
        let html = leaderboard_items(&rows);
        assert!(html.contains("&lt;script&gt;"));
        assert!(html.contains("2 drinks"));
        assert!(!html.contains("<script>"));
    }

    #[test]
    fn test_leaderboard_items_empty() {
        assert!(leaderboard_items(&[]).contains("Nobody here yet"));
    }
}
```

- [ ] **Step 3: Add event db tests**

In the `tests` module of `drinkinggame/src/db.rs`:

```rust
    async fn seed_room_with_players(pool: &DbPool) -> (i64, i64, i64) {
        let a = insert_player(pool, "alice", "h").await.unwrap();
        let b = insert_player(pool, "bob", "h").await.unwrap();
        let room = crate::rooms::create_room_with_unique_code(pool).await;
        join_room(pool, room.id, a).await;
        join_room(pool, room.id, b).await;
        (room.id, a, b)
    }

    #[tokio::test]
    async fn test_leaderboard_fold_and_order() {
        let pool = test_pool().await;
        let (room, alice, bob) = seed_room_with_players(&pool).await;
        insert_event(&pool, room, alice, "drink").await;
        insert_event(&pool, room, alice, "shot").await;
        insert_event(&pool, room, bob, "drink").await;

        let lb = leaderboard(&pool, room).await;
        assert_eq!(lb.len(), 2);
        assert_eq!((lb[0].name.as_str(), lb[0].drinks, lb[0].shots), ("alice", 1, 1));
        assert_eq!((lb[1].name.as_str(), lb[1].drinks, lb[1].shots), ("bob", 1, 0));
    }

    #[tokio::test]
    async fn test_members_with_no_events_appear_with_zeros() {
        let pool = test_pool().await;
        let (room, _alice, _bob) = seed_room_with_players(&pool).await;
        let lb = leaderboard(&pool, room).await;
        assert_eq!(lb.len(), 2);
        assert!(lb.iter().all(|r| r.drinks == 0 && r.shots == 0));
    }

    #[tokio::test]
    async fn test_undo_tombstones_latest_only() {
        let pool = test_pool().await;
        let (room, alice, _bob) = seed_room_with_players(&pool).await;
        insert_event(&pool, room, alice, "drink").await;
        insert_event(&pool, room, alice, "shot").await;

        assert!(undo_last_event(&pool, room, alice).await); // kills the shot
        let lb = leaderboard(&pool, room).await;
        let a = lb.iter().find(|r| r.name == "alice").unwrap();
        assert_eq!((a.drinks, a.shots), (1, 0));

        assert!(undo_last_event(&pool, room, alice).await); // kills the drink
        assert!(!undo_last_event(&pool, room, alice).await); // nothing left

        // Rows still exist — tombstoned, not deleted.
        let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM events WHERE room_id = ?1")
            .bind(room)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(total.0, 2);
    }

    #[tokio::test]
    async fn test_lifetime_counts_span_rooms_and_respect_undo() {
        let pool = test_pool().await;
        let alice = insert_player(&pool, "alice", "h").await.unwrap();
        let r1 = crate::rooms::create_room_with_unique_code(&pool).await;
        let r2 = crate::rooms::create_room_with_unique_code(&pool).await;
        join_room(&pool, r1.id, alice).await;
        join_room(&pool, r2.id, alice).await;
        insert_event(&pool, r1.id, alice, "drink").await;
        insert_event(&pool, r2.id, alice, "drink").await;
        insert_event(&pool, r2.id, alice, "shot").await;
        undo_last_event(&pool, r2.id, alice).await; // removes the shot

        assert_eq!(lifetime_counts(&pool, alice).await, (2, 0));
    }
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p drinkinggame`
Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add drinkinggame/
git commit -m "feat(drinks): event log with tombstone undo and leaderboard fold"
```

---

### Task 6: Broadcast hub, GameState, router skeleton, assets, standalone bin

**Files:**
- Create: `drinkinggame/src/hub.rs`
- Create: `drinkinggame/src/routes.rs`
- Create: `drinkinggame/src/main.rs` (standalone bin)
- Create: `drinkinggame/assets/game.css`
- Create: `drinkinggame/templates/landing.html`
- Create: `drinkinggame/templates/error.html`
- Modify: `drinkinggame/src/lib.rs` (GameState, router functions)
- Test: `drinkinggame/tests/http.rs` (integration harness) + inline hub tests

**Interfaces:**
- Consumes: `db::*`, `Config`
- Produces:
  - `hub::RoomMessage` (`Leaderboard(String) | Ended`), `hub::RoomHub` with `new()`, `subscribe(room_id: i64) -> broadcast::Receiver<RoomMessage>`, `publish(room_id: i64, msg: RoomMessage)`, `remove(room_id: i64)`, `active_rooms() -> Vec<i64>`
  - `GameState { pool: db::DbPool, hub: hub::RoomHub, base_path: Arc<str> }` (Clone)
  - `pub async fn router(config: Config) -> axum::Router` and `pub fn router_with_pool(pool: db::DbPool, base_path: &str) -> axum::Router` (tests use the latter)
  - `routes::router() -> Router<GameState>`; GET `/` landing (logged-out view for now), GET `/assets/game.css`, GET `/assets/htmx.min.js`
  - test helper `test_app()` in `tests/http.rs`

- [ ] **Step 1: Write hub.rs**

Create `drinkinggame/src/hub.rs`:

```rust
//! Per-room broadcast channels. Every connected screen (player phone or
//! spectator) subscribes; any state change publishes a freshly rendered
//! leaderboard fragment.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

#[derive(Clone, Debug)]
pub enum RoomMessage {
    /// Rendered <li> rows for the leaderboard.
    Leaderboard(String),
    /// The room was ended; clients should leave.
    Ended,
}

#[derive(Clone, Default)]
pub struct RoomHub {
    // std Mutex, not tokio: we never await while holding the lock.
    inner: Arc<Mutex<HashMap<i64, broadcast::Sender<RoomMessage>>>>,
}

impl RoomHub {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn subscribe(&self, room_id: i64) -> broadcast::Receiver<RoomMessage> {
        let mut map = self.inner.lock().unwrap();
        map.entry(room_id)
            .or_insert_with(|| broadcast::channel(32).0)
            .subscribe()
    }

    /// Publishing to a room with no subscribers is a silent no-op.
    pub fn publish(&self, room_id: i64, msg: RoomMessage) {
        let map = self.inner.lock().unwrap();
        if let Some(tx) = map.get(&room_id) {
            let _ = tx.send(msg);
        }
    }

    pub fn remove(&self, room_id: i64) {
        self.inner.lock().unwrap().remove(&room_id);
    }

    pub fn active_rooms(&self) -> Vec<i64> {
        self.inner.lock().unwrap().keys().copied().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_subscribe_publish_remove() {
        let hub = RoomHub::new();
        let mut rx = hub.subscribe(1);
        hub.publish(1, RoomMessage::Leaderboard("<li>x</li>".into()));
        match rx.recv().await.unwrap() {
            RoomMessage::Leaderboard(html) => assert_eq!(html, "<li>x</li>"),
            other => panic!("unexpected message: {other:?}"),
        }
        assert_eq!(hub.active_rooms(), vec![1]);
        hub.remove(1);
        assert!(hub.active_rooms().is_empty());
        hub.publish(1, RoomMessage::Ended); // no subscribers — must not panic
    }
}
```

- [ ] **Step 2: Write the CSS**

Create `drinkinggame/assets/game.css`:

```css
/* Drinking game — standalone stylesheet, deliberately independent of the
   portfolio's style.css. Phone-first, drunk-proof: huge targets, high
   contrast, nothing precise. */
* { box-sizing: border-box; margin: 0; }
:root {
  --bg: #14121a; --panel: #201d2a; --text: #f2eef8;
  --accent: #b48ef7; --danger: #f7768e; --muted: #8d87a0;
}
body {
  background: var(--bg); color: var(--text);
  font-family: system-ui, sans-serif;
  min-height: 100vh; padding: 1rem;
  display: flex; flex-direction: column; align-items: center; gap: 1rem;
}
h1 { font-size: 1.4rem; }
.room-code { font-size: 3rem; letter-spacing: 0.3em; font-weight: 800; }
.screen .room-code { font-size: 6rem; }
form, .panel {
  background: var(--panel); border-radius: 16px;
  padding: 1rem; width: 100%; max-width: 420px;
  display: flex; flex-direction: column; gap: 0.75rem;
}
input {
  font-size: 1.3rem; padding: 0.8rem; border-radius: 10px;
  border: 1px solid var(--muted); background: var(--bg); color: var(--text);
  width: 100%;
}
button {
  font-size: 1.5rem; font-weight: 700; padding: 1.1rem;
  border: none; border-radius: 12px; cursor: pointer;
  background: var(--accent); color: #191624;
}
button:active { transform: scale(0.97); }
.btn-row { display: grid; grid-template-columns: 1fr 1fr; gap: 0.75rem; width: 100%; max-width: 420px; }
.btn-big { min-height: 5.5rem; }
.btn-undo, .btn-end { background: transparent; color: var(--muted); border: 1px solid var(--muted); font-size: 1rem; padding: 0.6rem; }
.btn-end { color: var(--danger); border-color: var(--danger); }
#leaderboard { list-style: none; width: 100%; max-width: 420px; display: flex; flex-direction: column; gap: 0.5rem; padding: 0; }
#leaderboard li {
  display: flex; justify-content: space-between; align-items: baseline;
  background: var(--panel); border-radius: 10px; padding: 0.7rem 1rem;
}
#leaderboard li:first-child { border: 1px solid var(--accent); }
.lb-name { font-weight: 700; font-size: 1.1rem; }
.lb-counts { color: var(--muted); font-size: 0.95rem; }
.lb-empty { justify-content: center; color: var(--muted); }
.error { color: var(--danger); }
.lifetime { color: var(--muted); font-size: 0.9rem; }
a { color: var(--accent); }
```

- [ ] **Step 3: Write the landing and error templates**

Create `drinkinggame/templates/landing.html`:

```html
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Drinks</title>
  <link rel="stylesheet" href="{{ base_path }}/assets/game.css">
</head>
<body>
  <h1>Drinks</h1>
  {% if logged_in %}
  <p>Hey, <strong>{{ player_name }}</strong>.
     <span class="lifetime">Lifetime: {{ lifetime_drinks }} drinks &middot; {{ lifetime_shots }} shots</span></p>
  <form method="post" action="{{ base_path }}/rooms">
    <button type="submit">Start a night</button>
  </form>
  <form method="post" action="{{ base_path }}/join">
    <input name="code" placeholder="ROOM CODE" maxlength="4" autocapitalize="characters"
           autocomplete="off" required>
    <button type="submit">Join</button>
  </form>
  {% else %}
  <form method="post" action="{{ base_path }}/login">
    <input name="name" placeholder="Your name" maxlength="20" required>
    <input name="pin" placeholder="4-digit PIN" inputmode="numeric" pattern="[0-9]{4}"
           maxlength="4" required>
    <button type="submit">Let's go</button>
    <p class="lifetime">New name? This registers it. Known name? PIN logs you in from any phone.</p>
  </form>
  {% endif %}
</body>
</html>
```

Create `drinkinggame/templates/error.html`:

```html
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Oops</title>
  <link rel="stylesheet" href="{{ base_path }}/assets/game.css">
</head>
<body>
  <h1>Oops</h1>
  <p class="error">{{ message }}</p>
  <p><a href="{{ base_path }}/">Back home</a></p>
</body>
</html>
```

- [ ] **Step 4: Write routes.rs (skeleton: landing + assets + error page helper)**

Create `drinkinggame/src/routes.rs`:

```rust
use askama::Template;
use axum::extract::State;
use axum::http::header;
use axum::response::{Html, IntoResponse};
use axum::routing::get;
use axum::Router;

use crate::GameState;

#[derive(Template)]
#[template(path = "landing.html")]
struct LandingTemplate {
    base_path: String,
    logged_in: bool,
    player_name: String,
    lifetime_drinks: i64,
    lifetime_shots: i64,
}

#[derive(Template)]
#[template(path = "error.html")]
pub struct ErrorTemplate {
    base_path: String,
    message: String,
}

/// Full-page friendly error (unknown/ended room, etc.) with a link home.
pub fn error_page(state: &GameState, status: axum::http::StatusCode, message: &str) -> axum::response::Response {
    let tpl = ErrorTemplate {
        base_path: state.base_path.to_string(),
        message: message.to_string(),
    };
    (status, Html(tpl.render().unwrap())).into_response()
}

async fn landing(State(state): State<GameState>) -> impl IntoResponse {
    // Task 7 threads the session through; for now always the logged-out view.
    let tpl = LandingTemplate {
        base_path: state.base_path.to_string(),
        logged_in: false,
        player_name: String::new(),
        lifetime_drinks: 0,
        lifetime_shots: 0,
    };
    Html(tpl.render().unwrap())
}

async fn game_css() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "text/css")], include_str!("../assets/game.css"))
}

/// Single htmx copy for the whole repo: embed the portfolio's vendored file.
async fn htmx_js() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "application/javascript")],
        include_str!("../../static/htmx.min.js"),
    )
}

pub fn router() -> Router<GameState> {
    Router::new()
        .route("/", get(landing))
        .route("/assets/game.css", get(game_css))
        .route("/assets/htmx.min.js", get(htmx_js))
}
```

- [ ] **Step 5: Add GameState and router builders to lib.rs**

Replace the contents of `drinkinggame/src/lib.rs` with:

```rust
//! Drinking game — party drink tracker, mounted under /drinks in the
//! portfolio server or served standalone via the bin target.

pub mod auth;
pub mod db;
pub mod error;
pub mod hub;
pub mod models;
pub mod render;
pub mod rooms;
pub mod routes;

use std::sync::Arc;

/// Everything the crate needs from its host. No portfolio types leak in here.
pub struct Config {
    /// e.g. "sqlite:./drinkinggame.db"
    pub database_url: String,
    /// URL prefix the router is mounted under: "" standalone, "/drinks" nested.
    /// Used only for URL generation in templates/redirects — routing itself
    /// is prefix-agnostic because .nest_service() strips the prefix.
    pub base_path: String,
}

#[derive(Clone)]
pub struct GameState {
    pub pool: db::DbPool,
    pub hub: hub::RoomHub,
    pub base_path: Arc<str>,
}

/// Build the game as a self-contained, stateless Router — the crate owns its
/// pool and hub, so this composes with any host via .nest_service().
pub async fn router(config: Config) -> axum::Router {
    let pool = db::connect(&config.database_url).await;
    db::run_migrations(&pool).await;
    router_with_pool(pool, &config.base_path)
}

/// Test seam: integration tests inject an in-memory pool here.
pub fn router_with_pool(pool: db::DbPool, base_path: &str) -> axum::Router {
    let state = GameState {
        pool,
        hub: hub::RoomHub::new(),
        base_path: Arc::from(base_path),
    };
    routes::router().with_state(state)
}
```

- [ ] **Step 6: Write the standalone bin**

Create `drinkinggame/src/main.rs`:

```rust
//! Standalone dev server: `cargo run -p drinkinggame` then open
//! http://localhost:3001 — no portfolio, no nginx, base_path is "".

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    dotenvy::dotenv().ok();

    let config = drinkinggame::Config {
        database_url: std::env::var("DRINKS_DATABASE_URL")
            .unwrap_or_else(|_| "sqlite:./drinkinggame.db".to_string()),
        base_path: String::new(),
    };
    let app = drinkinggame::router(config).await;

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3001").await.unwrap();
    tracing::info!("drinkinggame standalone on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}
```

- [ ] **Step 7: Write the integration test harness**

Create `drinkinggame/tests/http.rs`:

```rust
//! Handler-level integration tests via tower::ServiceExt, portfolio-style.

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use sqlx::sqlite::SqlitePoolOptions;
use tower::ServiceExt;

async fn test_app() -> Router {
    // max_connections(1): a :memory: db exists per-connection.
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    drinkinggame::db::run_migrations(&pool).await;
    drinkinggame::router_with_pool(pool, "")
}

async fn body_string(res: axum::response::Response) -> String {
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    String::from_utf8(bytes.to_vec()).unwrap()
}

#[tokio::test]
async fn test_landing_serves_login_form() {
    let app = test_app().await;
    let res = app
        .oneshot(Request::get("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let html = body_string(res).await;
    assert!(html.contains(r#"action="/login""#));
    assert!(html.contains("4-digit PIN"));
}

#[tokio::test]
async fn test_assets_are_served() {
    let app = test_app().await;
    for (path, ct) in [
        ("/assets/game.css", "text/css"),
        ("/assets/htmx.min.js", "application/javascript"),
    ] {
        let res = app
            .clone()
            .oneshot(Request::get(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK, "{path}");
        assert_eq!(res.headers()[header::CONTENT_TYPE], ct, "{path}");
    }
}
```

- [ ] **Step 8: Run everything**

Run: `cargo test -p drinkinggame && cargo run -p drinkinggame &` then `curl -s localhost:3001/ | grep -q "4-digit PIN" && echo OK; kill %1`
Expected: tests PASS; curl prints OK (standalone bin boots and serves).

- [ ] **Step 9: Commit**

```bash
git add drinkinggame/
git commit -m "feat(drinks): broadcast hub, router skeleton, landing page, standalone bin"
```

---

### Task 7: Login route + session extractors + logged-in landing

**Files:**
- Modify: `drinkinggame/src/auth.rs` (extractors)
- Modify: `drinkinggame/src/routes.rs` (login handler, landing uses session)
- Test: `drinkinggame/tests/http.rs`

**Interfaces:**
- Consumes: `auth::login_or_register`, `db::create_session`, `db::get_session_player`, `db::lifetime_counts`
- Produces:
  - `auth::PlayerSession(pub Player)` — extractor; rejection = redirect to `{base_path}/`
  - `auth::OptionalPlayer(pub Option<Player>)` — never rejects
  - `POST /login` (form `name`, `pin`) → sets `dg_session` cookie, 303 → `{base_path}/`
  - Landing shows name + lifetime stats + create/join forms when logged in

- [ ] **Step 1: Add extractors to auth.rs**

Append to `drinkinggame/src/auth.rs`:

```rust
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::response::{IntoResponse, Redirect};

use crate::GameState;

fn extract_session_cookie(parts: &Parts) -> Option<String> {
    let cookies = parts.headers.get("cookie")?.to_str().ok()?;
    for cookie in cookies.split(';') {
        if let Some(val) = cookie.trim().strip_prefix(&format!("{COOKIE_NAME}=")) {
            return Some(val.to_string());
        }
    }
    None
}

/// Requires a live session; otherwise redirects to the game landing page.
pub struct PlayerSession(pub Player);

impl FromRequestParts<GameState> for PlayerSession {
    type Rejection = axum::response::Response;

    async fn from_request_parts(parts: &mut Parts, state: &GameState) -> Result<Self, Self::Rejection> {
        if let Some(id) = extract_session_cookie(parts) {
            if let Some(player) = db::get_session_player(&state.pool, &id).await {
                return Ok(PlayerSession(player));
            }
        }
        Err(Redirect::to(&format!("{}/", state.base_path)).into_response())
    }
}

/// Checks the session without ever rejecting.
pub struct OptionalPlayer(pub Option<Player>);

impl FromRequestParts<GameState> for OptionalPlayer {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, state: &GameState) -> Result<Self, Self::Rejection> {
        let player = match extract_session_cookie(parts) {
            Some(id) => db::get_session_player(&state.pool, &id).await,
            None => None,
        };
        Ok(OptionalPlayer(player))
    }
}
```

- [ ] **Step 2: Add the login handler and session-aware landing to routes.rs**

In `drinkinggame/src/routes.rs`, add imports:

```rust
use axum::extract::Form;
use axum::response::Redirect;
use axum::routing::post;
use serde::Deserialize;

use crate::auth::{self, OptionalPlayer};
use crate::db;
use crate::error::GameError;
```

Replace the `landing` handler:

```rust
async fn landing(
    State(state): State<GameState>,
    OptionalPlayer(player): OptionalPlayer,
) -> impl IntoResponse {
    let tpl = match player {
        Some(p) => {
            let (drinks, shots) = db::lifetime_counts(&state.pool, p.id).await;
            LandingTemplate {
                base_path: state.base_path.to_string(),
                logged_in: true,
                player_name: p.name,
                lifetime_drinks: drinks,
                lifetime_shots: shots,
            }
        }
        None => LandingTemplate {
            base_path: state.base_path.to_string(),
            logged_in: false,
            player_name: String::new(),
            lifetime_drinks: 0,
            lifetime_shots: 0,
        },
    };
    Html(tpl.render().unwrap())
}
```

Add the login handler:

```rust
#[derive(Deserialize)]
struct LoginForm {
    name: String,
    pin: String,
}

async fn login(
    State(state): State<GameState>,
    Form(form): Form<LoginForm>,
) -> axum::response::Response {
    match auth::login_or_register(&state.pool, &form.name, &form.pin).await {
        Ok(player) => {
            let sid = auth::new_session_id();
            db::create_session(&state.pool, &sid, player.id, "+90 days").await;
            (
                [(header::SET_COOKIE, auth::session_cookie(&sid))],
                Redirect::to(&format!("{}/", state.base_path)),
            )
                .into_response()
        }
        // The login form is a plain (non-HTMX) post, so render the friendly
        // full error page — a bare fragment would arrive unstyled.
        Err(e @ GameError::WrongPin) => {
            error_page(&state, axum::http::StatusCode::UNAUTHORIZED, &e.to_string())
        }
        Err(e @ (GameError::InvalidName | GameError::InvalidPin)) => {
            error_page(&state, axum::http::StatusCode::UNPROCESSABLE_ENTITY, &e.to_string())
        }
        Err(e) => e.into_response(),
    }
}
```

Register in `router()`:

```rust
        .route("/login", post(login))
```

- [ ] **Step 3: Add integration tests**

Append to `drinkinggame/tests/http.rs`:

```rust
/// Logs in (registering on first use) and returns the "dg_session=..." pair.
async fn login(app: &Router, name: &str, pin: &str) -> String {
    let res = app
        .clone()
        .oneshot(
            Request::post("/login")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(format!("name={name}&pin={pin}")))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    res.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string()
}

#[tokio::test]
async fn test_login_sets_cookie_and_landing_recognizes_it() {
    let app = test_app().await;
    let cookie = login(&app, "hampter", "1234").await;
    assert!(cookie.starts_with("dg_session="));

    let res = app
        .oneshot(
            Request::get("/")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let html = body_string(res).await;
    assert!(html.contains("hampter"));
    assert!(html.contains("Lifetime: 0 drinks"));
}

#[tokio::test]
async fn test_wrong_pin_is_rejected() {
    let app = test_app().await;
    login(&app, "hampter", "1234").await;
    let res = app
        .oneshot(
            Request::post("/login")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from("name=hampter&pin=9999"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    assert!(body_string(res).await.contains("wrong PIN"));
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p drinkinggame`
Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add drinkinggame/
git commit -m "feat(drinks): login route, session extractors, personalized landing"
```

---

### Task 8: Room routes — create, join, player room page

**Files:**
- Create: `drinkinggame/templates/room.html`
- Modify: `drinkinggame/src/routes.rs`
- Test: `drinkinggame/tests/http.rs`

**Interfaces:**
- Consumes: `auth::PlayerSession`, `rooms::create_room_with_unique_code`, `db::get_open_room`, `db::join_room`, `db::leaderboard`, `render::leaderboard_items`, `error_page`
- Produces:
  - `POST /rooms` → creates room, joins creator, 303 → `{base_path}/room/{code}`
  - `POST /join` (form `code`) → 303 → `{base_path}/room/{CODE}` (uppercased) or friendly error page
  - `GET /room/{code}` → player view; **visiting auto-joins** (a shared URL is a join link); dead code → friendly error page
  - `RoomTemplate { base_path, code, player_name, leaderboard_items }`

- [ ] **Step 1: Write room.html**

Create `drinkinggame/templates/room.html` (the SSE `<script>` is included now; the endpoint lands in Task 10 — until then the browser retries silently, which tests never see):

```html
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Room {{ code }}</title>
  <link rel="stylesheet" href="{{ base_path }}/assets/game.css">
  <script src="{{ base_path }}/assets/htmx.min.js" defer></script>
</head>
<body>
  <p class="room-code">{{ code }}</p>
  <p>Drinking as <strong>{{ player_name }}</strong></p>

  <div class="btn-row">
    <button class="btn-big" hx-post="{{ base_path }}/room/{{ code }}/event"
            hx-vals='{"kind":"drink"}' hx-swap="none">+1 Drink</button>
    <button class="btn-big" hx-post="{{ base_path }}/room/{{ code }}/event"
            hx-vals='{"kind":"shot"}' hx-swap="none">+1 Shot</button>
  </div>
  <div class="btn-row">
    <button class="btn-undo" hx-post="{{ base_path }}/room/{{ code }}/undo"
            hx-swap="none">Undo</button>
    <form method="post" action="{{ base_path }}/room/{{ code }}/end"
          onsubmit="return confirm('End the night for everyone?')">
      <button type="submit" class="btn-end">End night</button>
    </form>
  </div>

  <ol id="leaderboard">{{ leaderboard_items|safe }}</ol>

  <script>
    const es = new EventSource("{{ base_path }}/room/{{ code }}/sse");
    es.addEventListener("leaderboard", (e) => {
      document.getElementById("leaderboard").innerHTML = e.data;
    });
    es.addEventListener("ended", () => {
      es.close();
      window.location = "{{ base_path }}/";
    });
  </script>
</body>
</html>
```

- [ ] **Step 2: Add handlers to routes.rs**

Add imports: `use axum::extract::Path;`, `use axum::http::StatusCode;`, `use crate::auth::PlayerSession;`, `use crate::render;`, `use crate::rooms;`.

Add template struct and handlers:

```rust
#[derive(Template)]
#[template(path = "room.html")]
struct RoomTemplate {
    base_path: String,
    code: String,
    player_name: String,
    leaderboard_items: String,
}

async fn create_room(
    State(state): State<GameState>,
    PlayerSession(player): PlayerSession,
) -> impl IntoResponse {
    let room = rooms::create_room_with_unique_code(&state.pool).await;
    db::join_room(&state.pool, room.id, player.id).await;
    Redirect::to(&format!("{}/room/{}", state.base_path, room.code))
}

#[derive(Deserialize)]
struct JoinForm {
    code: String,
}

async fn join_room_handler(
    State(state): State<GameState>,
    PlayerSession(_player): PlayerSession,
    Form(form): Form<JoinForm>,
) -> axum::response::Response {
    let code = form.code.trim().to_uppercase();
    match db::get_open_room(&state.pool, &code).await {
        // The room page itself performs the join — one code path for both
        // form joins and shared-link joins.
        Some(_) => Redirect::to(&format!("{}/room/{code}", state.base_path)).into_response(),
        None => error_page(&state, StatusCode::NOT_FOUND, "No open room with that code"),
    }
}

async fn room_page(
    State(state): State<GameState>,
    PlayerSession(player): PlayerSession,
    Path(code): Path<String>,
) -> axum::response::Response {
    let code = code.to_uppercase();
    let Some(room) = db::get_open_room(&state.pool, &code).await else {
        return error_page(&state, StatusCode::NOT_FOUND, "Room not found or already ended");
    };
    // Visiting a room joins it: room URLs double as invite links.
    db::join_room(&state.pool, room.id, player.id).await;
    let rows = db::leaderboard(&state.pool, room.id).await;
    let tpl = RoomTemplate {
        base_path: state.base_path.to_string(),
        code,
        player_name: player.name,
        leaderboard_items: render::leaderboard_items(&rows),
    };
    Html(tpl.render().unwrap()).into_response()
}
```

Register in `router()`:

```rust
        .route("/rooms", post(create_room))
        .route("/join", post(join_room_handler))
        .route("/room/{code}", get(room_page))
```

- [ ] **Step 3: Add integration tests**

Append to `drinkinggame/tests/http.rs`:

```rust
/// Creates a room as `cookie`'s player and returns the room code.
async fn create_room(app: &Router, cookie: &str) -> String {
    let res = app
        .clone()
        .oneshot(
            Request::post("/rooms")
                .header(header::COOKIE, cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let loc = res.headers()[header::LOCATION].to_str().unwrap();
    loc.rsplit('/').next().unwrap().to_string()
}

#[tokio::test]
async fn test_create_room_and_view_it() {
    let app = test_app().await;
    let cookie = login(&app, "alice", "1234").await;
    let code = create_room(&app, &cookie).await;
    assert_eq!(code.len(), 4);

    let res = app
        .oneshot(
            Request::get(&format!("/room/{code}"))
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let html = body_string(res).await;
    assert!(html.contains("+1 Drink"));
    assert!(html.contains("alice")); // creator is on the leaderboard
}

#[tokio::test]
async fn test_visiting_room_auto_joins() {
    let app = test_app().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let code = create_room(&app, &alice).await;

    // Bob opens the shared link — no explicit join step.
    let res = app
        .clone()
        .oneshot(
            Request::get(&format!("/room/{code}"))
                .header(header::COOKIE, &bob)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let html = body_string(res).await;
    assert!(html.contains("alice") && html.contains("bob"));
}

#[tokio::test]
async fn test_room_requires_session_and_dead_codes_are_friendly() {
    let app = test_app().await;
    // No cookie -> redirected to landing.
    let res = app
        .clone()
        .oneshot(Request::get("/room/XXXX").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    // Cookie but nonexistent room -> friendly 404 page with a home link.
    let cookie = login(&app, "alice", "1234").await;
    let res = app
        .oneshot(
            Request::get("/room/XXXX")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert!(body_string(res).await.contains("Back home"));
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p drinkinggame`
Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add drinkinggame/
git commit -m "feat(drinks): room create/join and player room page with auto-join"
```

---

### Task 9: Event, undo and end handlers + broadcast + mechanics hook

**Files:**
- Create: `drinkinggame/src/mechanics.rs`
- Modify: `drinkinggame/src/routes.rs`
- Modify: `drinkinggame/src/lib.rs` (`pub mod mechanics;`)
- Test: `drinkinggame/tests/http.rs`

**Interfaces:**
- Consumes: `db::insert_event`, `db::undo_last_event`, `db::touch_room`, `db::end_room`, `db::leaderboard`, `hub::RoomHub`, `render::leaderboard_items`
- Produces:
  - `POST /room/{code}/event` (form `kind` = `drink|shot`) → 204, broadcasts fresh leaderboard
  - `POST /room/{code}/undo` → 204, broadcasts
  - `POST /room/{code}/end` → broadcasts `RoomMessage::Ended`, drops the channel, 303 home
  - `mechanics::on_event(room_id: i64, player_id: i64, kind: &str)` — v1 no-op hook, called after every logged event
  - `routes::broadcast_leaderboard(state: &GameState, room_id: i64)` (pub(crate), reused by SSE task)

- [ ] **Step 1: Write mechanics.rs (the extension point)**

Create `drinkinggame/src/mechanics.rs`:

```rust
//! Idle-mechanics extension point. V1 ships this empty on purpose — future
//! mechanics fold over the append-only event log and publish through the
//! same broadcast hub the leaderboard uses.

use crate::GameState;

/// Called after every successfully logged event.
pub fn on_event(_room_id: i64, _player_id: i64, _kind: &str) {}

/// One global 1 Hz ticker iterating rooms with live subscribers — bounded
/// work, no per-room task spawning. Spawned by router_with_pool (Task 11).
pub fn spawn_ticker(state: GameState) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
        loop {
            interval.tick().await;
            for room_id in state.hub.active_rooms() {
                tick(&state, room_id).await;
            }
        }
    });
}

async fn tick(_state: &GameState, _room_id: i64) {
    // v1: nothing. Point-like mechanics land here without schema changes.
}
```

Add `pub mod mechanics;` to `drinkinggame/src/lib.rs`.

- [ ] **Step 2: Add the mutation handlers to routes.rs**

```rust
/// Re-render the standings and push to every subscribed screen in the room.
pub(crate) async fn broadcast_leaderboard(state: &GameState, room_id: i64) {
    let rows = db::leaderboard(&state.pool, room_id).await;
    state
        .hub
        .publish(room_id, crate::hub::RoomMessage::Leaderboard(render::leaderboard_items(&rows)));
}

#[derive(Deserialize)]
struct EventForm {
    kind: String,
}

async fn log_event(
    State(state): State<GameState>,
    PlayerSession(player): PlayerSession,
    Path(code): Path<String>,
    Form(form): Form<EventForm>,
) -> axum::response::Response {
    if form.kind != "drink" && form.kind != "shot" {
        return StatusCode::UNPROCESSABLE_ENTITY.into_response();
    }
    let Some(room) = db::get_open_room(&state.pool, &code.to_uppercase()).await else {
        return GameError::RoomNotFound.into_response();
    };
    db::insert_event(&state.pool, room.id, player.id, &form.kind).await;
    db::touch_room(&state.pool, room.id).await;
    crate::mechanics::on_event(room.id, player.id, &form.kind);
    broadcast_leaderboard(&state, room.id).await;
    StatusCode::NO_CONTENT.into_response()
}

async fn undo_event(
    State(state): State<GameState>,
    PlayerSession(player): PlayerSession,
    Path(code): Path<String>,
) -> axum::response::Response {
    let Some(room) = db::get_open_room(&state.pool, &code.to_uppercase()).await else {
        return GameError::RoomNotFound.into_response();
    };
    if db::undo_last_event(&state.pool, room.id, player.id).await {
        db::touch_room(&state.pool, room.id).await;
        broadcast_leaderboard(&state, room.id).await;
    }
    StatusCode::NO_CONTENT.into_response()
}

async fn end_room_handler(
    State(state): State<GameState>,
    PlayerSession(_player): PlayerSession,
    Path(code): Path<String>,
) -> axum::response::Response {
    let Some(room) = db::get_open_room(&state.pool, &code.to_uppercase()).await else {
        return GameError::RoomNotFound.into_response();
    };
    db::end_room(&state.pool, room.id).await;
    state.hub.publish(room.id, crate::hub::RoomMessage::Ended);
    state.hub.remove(room.id);
    Redirect::to(&format!("{}/", state.base_path)).into_response()
}
```

Register in `router()`:

```rust
        .route("/room/{code}/event", post(log_event))
        .route("/room/{code}/undo", post(undo_event))
        .route("/room/{code}/end", post(end_room_handler))
```

- [ ] **Step 3: Add integration tests**

Append to `drinkinggame/tests/http.rs`:

```rust
async fn post_form(app: &Router, cookie: &str, path: &str, body: &str) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::post(path)
                .header(header::COOKIE, cookie)
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn room_page_html(app: &Router, cookie: &str, code: &str) -> String {
    let res = app
        .clone()
        .oneshot(
            Request::get(&format!("/room/{code}"))
                .header(header::COOKIE, cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    body_string(res).await
}

#[tokio::test]
async fn test_log_undo_and_leaderboard_counts() {
    let app = test_app().await;
    let cookie = login(&app, "alice", "1234").await;
    let code = create_room(&app, &cookie).await;

    let res = post_form(&app, &cookie, &format!("/room/{code}/event"), "kind=drink").await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    post_form(&app, &cookie, &format!("/room/{code}/event"), "kind=shot").await;
    assert!(room_page_html(&app, &cookie, &code).await.contains("1 drinks &middot; 1 shots"));

    post_form(&app, &cookie, &format!("/room/{code}/undo"), "").await;
    assert!(room_page_html(&app, &cookie, &code).await.contains("1 drinks &middot; 0 shots"));

    // Junk kind is rejected.
    let res = post_form(&app, &cookie, &format!("/room/{code}/event"), "kind=beer").await;
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_end_room_closes_it_for_everyone() {
    let app = test_app().await;
    let cookie = login(&app, "alice", "1234").await;
    let code = create_room(&app, &cookie).await;

    let res = post_form(&app, &cookie, &format!("/room/{code}/end"), "").await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    // Room page is now a friendly 404; logging is rejected too.
    let res = app
        .clone()
        .oneshot(
            Request::get(&format!("/room/{code}"))
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    let res = post_form(&app, &cookie, &format!("/room/{code}/event"), "kind=drink").await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p drinkinggame`
Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add drinkinggame/
git commit -m "feat(drinks): drink/shot logging, undo, room end with broadcast + mechanics hook"
```

---

### Task 10: SSE stream + spectator screen

**Files:**
- Create: `drinkinggame/templates/screen.html`
- Modify: `drinkinggame/src/routes.rs`
- Test: `drinkinggame/tests/http.rs`

**Interfaces:**
- Consumes: `hub::RoomHub::subscribe`, `broadcast_leaderboard` internals (`db::leaderboard` + `render::leaderboard_items`)
- Produces:
  - `GET /room/{code}/sse` — `text/event-stream`, no auth (room code is the gate, same as the spectator page); named events `leaderboard` (fragment payload) and `ended`; sends `X-Accel-Buffering: no`; first message is the current standings so a reconnecting phone is instantly correct
  - `GET /room/{code}/screen` — unauthenticated spectator page, `ScreenTemplate { base_path, code, leaderboard_items }`

- [ ] **Step 1: Write screen.html**

Create `drinkinggame/templates/screen.html`:

```html
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Room {{ code }} — Big Screen</title>
  <link rel="stylesheet" href="{{ base_path }}/assets/game.css">
</head>
<body class="screen">
  <p>Join at <strong>{{ base_path }}/</strong> with code</p>
  <p class="room-code">{{ code }}</p>
  <ol id="leaderboard">{{ leaderboard_items|safe }}</ol>
  <script>
    const es = new EventSource("{{ base_path }}/room/{{ code }}/sse");
    es.addEventListener("leaderboard", (e) => {
      document.getElementById("leaderboard").innerHTML = e.data;
    });
    es.addEventListener("ended", () => {
      es.close();
      document.getElementById("leaderboard").innerHTML =
        '<li class="lb-empty">Night over</li>';
    });
  </script>
</body>
</html>
```

- [ ] **Step 2: Add SSE + screen handlers to routes.rs**

Add imports:

```rust
use axum::response::sse::{Event, KeepAlive, Sse};
use futures::StreamExt;
use std::convert::Infallible;
use tokio_stream::wrappers::BroadcastStream;

use crate::hub::RoomMessage;
```

Add handlers:

```rust
async fn sse_stream(
    State(state): State<GameState>,
    Path(code): Path<String>,
) -> axum::response::Response {
    let Some(room) = db::get_open_room(&state.pool, &code.to_uppercase()).await else {
        return GameError::RoomNotFound.into_response();
    };

    // Subscribe BEFORE rendering the snapshot — no update can slip between.
    let rx = state.hub.subscribe(room.id);
    let rows = db::leaderboard(&state.pool, room.id).await;
    let initial = render::leaderboard_items(&rows);

    let stream = futures::stream::once(async move {
        Ok::<_, Infallible>(Event::default().event("leaderboard").data(initial))
    })
    .chain(BroadcastStream::new(rx).filter_map(|msg| async move {
        match msg {
            Ok(RoomMessage::Leaderboard(html)) => {
                Some(Ok(Event::default().event("leaderboard").data(html)))
            }
            Ok(RoomMessage::Ended) => Some(Ok(Event::default().event("ended").data(""))),
            // Lagged receiver: skip — the next update carries full state anyway.
            Err(_) => None,
        }
    }));

    (
        // Belt-and-braces alongside nginx's proxy_buffering off.
        [(header::HeaderName::from_static("x-accel-buffering"), "no")],
        Sse::new(stream).keep_alive(KeepAlive::default()),
    )
        .into_response()
}

#[derive(Template)]
#[template(path = "screen.html")]
struct ScreenTemplate {
    base_path: String,
    code: String,
    leaderboard_items: String,
}

async fn screen_page(
    State(state): State<GameState>,
    Path(code): Path<String>,
) -> axum::response::Response {
    let code = code.to_uppercase();
    let Some(room) = db::get_open_room(&state.pool, &code).await else {
        return error_page(&state, StatusCode::NOT_FOUND, "Room not found or already ended");
    };
    let rows = db::leaderboard(&state.pool, room.id).await;
    let tpl = ScreenTemplate {
        base_path: state.base_path.to_string(),
        code,
        leaderboard_items: render::leaderboard_items(&rows),
    };
    Html(tpl.render().unwrap()).into_response()
}
```

Register in `router()`:

```rust
        .route("/room/{code}/sse", get(sse_stream))
        .route("/room/{code}/screen", get(screen_page))
```

- [ ] **Step 3: Add integration tests**

Append to `drinkinggame/tests/http.rs`:

```rust
#[tokio::test]
async fn test_screen_is_public() {
    let app = test_app().await;
    let cookie = login(&app, "alice", "1234").await;
    let code = create_room(&app, &cookie).await;

    // No cookie at all — spectator view must render.
    let res = app
        .oneshot(
            Request::get(&format!("/room/{code}/screen"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let html = body_string(res).await;
    assert!(html.contains(&code));
    assert!(html.contains("alice"));
}

#[tokio::test]
async fn test_sse_endpoint_streams_event_stream() {
    use futures::StreamExt; // for .next() on the body data stream
    let app = test_app().await;
    let cookie = login(&app, "alice", "1234").await;
    let code = create_room(&app, &cookie).await;

    let res = app
        .oneshot(
            Request::get(&format!("/room/{code}/sse"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert!(res.headers()[header::CONTENT_TYPE]
        .to_str()
        .unwrap()
        .starts_with("text/event-stream"));
    assert_eq!(res.headers()["x-accel-buffering"], "no");
    // Don't collect the body — it's an infinite stream. Read one frame:
    let mut body = res.into_body().into_data_stream();
    let first = body.next().await.unwrap().unwrap();
    let text = String::from_utf8(first.to_vec()).unwrap();
    assert!(text.contains("event: leaderboard"));
    assert!(text.contains("alice"));
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p drinkinggame`
Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add drinkinggame/
git commit -m "feat(drinks): SSE leaderboard stream and public spectator screen"
```

---

### Task 11: Background tasks — session sweep, inactive-room ending, ticker

**Files:**
- Modify: `drinkinggame/src/lib.rs`
- Test: covered by existing unit tests (`test_session_roundtrip_and_expiry`, `test_end_inactive_rooms`); this task wires them to a timer

**Interfaces:**
- Consumes: `db::cleanup_expired_sessions`, `db::end_inactive_rooms`, `hub::remove`, `mechanics::spawn_ticker`
- Produces: `router_with_pool` spawns both loops; `MAX_IDLE_HOURS: i64 = 12` const in lib.rs

- [ ] **Step 1: Add the cleanup loop and ticker to router_with_pool**

In `drinkinggame/src/lib.rs`:

```rust
/// Rooms idle longer than this are ended by the hourly sweep.
pub const MAX_IDLE_HOURS: i64 = 12;

fn spawn_cleanup(state: GameState) {
    tokio::spawn(async move {
        // Hourly, mirroring the portfolio's cleanup cadence.
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600));
        loop {
            interval.tick().await;
            db::cleanup_expired_sessions(&state.pool).await;
            for room_id in db::end_inactive_rooms(&state.pool, MAX_IDLE_HOURS).await {
                state.hub.publish(room_id, hub::RoomMessage::Ended);
                state.hub.remove(room_id);
                tracing::info!("ended inactive room {room_id}");
            }
        }
    });
}
```

And change `router_with_pool` to spawn both before returning:

```rust
pub fn router_with_pool(pool: db::DbPool, base_path: &str) -> axum::Router {
    let state = GameState {
        pool,
        hub: hub::RoomHub::new(),
        base_path: Arc::from(base_path),
    };
    spawn_cleanup(state.clone());
    mechanics::spawn_ticker(state.clone());
    routes::router().with_state(state)
}
```

- [ ] **Step 2: Run the full suite (the loops now run inside every integration test app — they must not disturb anything)**

Run: `cargo test -p drinkinggame`
Expected: all PASS, no hangs (both loops only touch the db on their tick; the first hourly tick fires immediately and sweeps an empty table — harmless).

- [ ] **Step 3: Commit**

```bash
git add drinkinggame/
git commit -m "feat(drinks): hourly session/room cleanup and mechanics ticker"
```

---

### Task 12: Portfolio integration, deploy config, docs, browser verification

**Files:**
- Modify: `Cargo.toml` (root — add path dependency)
- Modify: `src/main.rs` (nest the game)
- Modify: `.env.example`
- Modify: `templates/hub/hub.html` (hub card)
- Modify: `static/palette.js` (COMMANDS entry)
- Modify: `deploy/nginx.conf` (SSE + rate-limit locations)
- Modify: `CLAUDE.md` (workspace, env var, deploy notes)

**Interfaces:**
- Consumes: `drinkinggame::router`, `drinkinggame::Config`
- Produces: game live at `/drinks` in the portfolio binary; all docs and deploy config updated.

- [ ] **Step 1: Add the dependency**

In the root `Cargo.toml` `[dependencies]` section:

```toml
drinkinggame = { path = "drinkinggame" }
```

- [ ] **Step 2: Mount the game in src/main.rs**

After the `webauthn` setup and before building `state`, add:

```rust
    // --- Drinking game (separate crate, own SQLite file, own sessions) ---
    // Built as a self-contained Router<()>; nest_service strips the /drinks
    // prefix while base_path makes the game generate /drinks/... URLs.
    let drinks = drinkinggame::router(drinkinggame::Config {
        database_url: std::env::var("DRINKS_DATABASE_URL")
            .unwrap_or_else(|_| "sqlite:./drinkinggame.db".to_string()),
        base_path: "/drinks".to_string(),
    })
    .await;
```

In the router chain, directly after `.merge(routes::tasks::router())`:

```rust
        .nest_service("/drinks", drinks)  // party drink tracker (drinkinggame crate)
```

(`.nest_service`, not `.nest` — the game router is a finished `Router<()>` while the outer router's state is `Arc<AppState>`, so `.nest` would not type-check.)

- [ ] **Step 3: Add the env var**

Append to `.env.example`:

```
# Drinking game — separate SQLite file from the portfolio DB
DRINKS_DATABASE_URL=sqlite:./drinkinggame.db
```

- [ ] **Step 4: Hub card + palette command**

In `templates/hub/hub.html`, next to the existing `/tasks` hub-card anchor, add (match the existing card markup exactly — copy the `/tasks` card's structure):

```html
  <a href="/drinks" class="hub-card">
    <h2>Drinks</h2>
    <p>Party night drink tracker — join with a room code.</p>
  </a>
```

In `static/palette.js`, add to `COMMANDS` after the "Go to Drawing Tasks" entry:

```js
  {
    label: 'Go to Drinking Game',
    keywords: ['drinks', 'drinking', 'party', 'game', 'shots', 'room'],
    action() { location.href = '/drinks'; },
  },
```

- [ ] **Step 5: nginx — SSE passthrough and PIN rate limit**

In `deploy/nginx.conf`, inside the HTTPS `server` block, BEFORE the generic `location /` block, add:

```nginx
    # Drinking game SSE — nginx must not buffer the stream or it never arrives.
    location ~ ^/drinks/room/[A-Z]+/sse$ {
        proxy_pass http://127.0.0.1:3000;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_http_version 1.1;
        proxy_set_header Connection "";
        proxy_buffering off;
        proxy_read_timeout 1h;
    }

    # Drinking game login — a 4-digit PIN is 10k combos; throttle guesses
    # with the same zone that protects /api/auth/.
    location = /drinks/login {
        limit_req zone=auth burst=5 nodelay;
        proxy_pass http://127.0.0.1:3000;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
    }
```

(The existing `limit_req_zone ... zone=auth:10m rate=10r/m;` at the top of the file already defines the zone — reuse it, do not add a second zone. Reminder from CLAUDE.md: nginx.conf is NOT deployed by CI — copy to `/etc/nginx/sites-available/portfolio`, `nginx -t`, reload manually.)

- [ ] **Step 6: Update CLAUDE.md**

- In the Commands section note: repo is now a cargo workspace; `cargo build` / `cargo test` at the root cover both crates; `cargo run -p drinkinggame` serves the game standalone on :3001.
- In the Environment table add: `DRINKS_DATABASE_URL` — SQLite path for the drinking game (separate file from the portfolio DB).
- In Architecture add one line: `/drinks` is the `drinkinggame` crate (own DB, own name+PIN sessions, SSE leaderboards) nested via `nest_service` in `main.rs`; its templates do NOT extend `base.html` (recorded exception).
- In Deployment note the two new manual nginx locations (SSE no-buffering; login rate limit) added in Step 5.
- Note the game crate uses runtime-checked sqlx queries — no `.sqlx` cache entries for it, `cargo sqlx prepare` remains portfolio-only.

- [ ] **Step 7: Full verification**

Run: `cargo fmt --check && cargo clippy && cargo test && SQLX_OFFLINE=true cargo build --release`
Expected: fmt clean for new files, clippy clean, ALL tests pass (portfolio 35 + drinkinggame suite), release build produces `target/release/drawingportfolio`.

- [ ] **Step 8: Manual browser verification (required — UI is not verified by tests)**

Run `cargo run` and check in a real browser:

1. `http://localhost:3000/drinks/` — landing renders, styles load (CSS via `/drinks/assets/game.css`).
2. Register a name + PIN, create a room — lands on `/drinks/room/XXXX` with big buttons.
3. Open `/drinks/room/XXXX/screen` in a second window. Press **+1 Drink** in the first — the screen updates live without reload (SSE through the nested path).
4. Second browser profile (or private window): register a second name, open the room URL directly — auto-joined, both names on both screens.
5. Undo removes the latest drink on all screens; **End night** returns the phone to the landing page and the screen shows "Night over".
6. Refresh a room page mid-game — counts are correct immediately (SSE snapshot-on-connect).
7. `http://localhost:3000/` hub shows the Drinks card; Ctrl+K palette navigates to `/drinks`.

- [ ] **Step 9: Commit**

```bash
git add Cargo.toml Cargo.lock src/main.rs .env.example templates/hub/hub.html static/palette.js deploy/nginx.conf CLAUDE.md
git commit -m "feat: mount drinking game at /drinks with nginx SSE + rate-limit config"
```

---

## Post-plan notes for the executor

- **Ordering:** Tasks are strictly sequential; each leaves `cargo test` green.
- **Deploy day (manual, after merge):** copy `deploy/nginx.conf` to the server, `nginx -t`, `systemctl reload nginx`, and add `DRINKS_DATABASE_URL=sqlite:///opt/portfolio/drinkinggame.db` to the server's `.env`. The systemd unit and deploy.yml need no changes.
- **Out of scope (per spec):** idle mechanics content, admin tools, portfolio SSO, PWA packaging, logout.
