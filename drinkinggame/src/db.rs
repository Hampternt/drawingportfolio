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
}
