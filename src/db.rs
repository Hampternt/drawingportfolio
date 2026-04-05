use sqlx::{SqlitePool, sqlite::{SqlitePoolOptions, SqliteConnectOptions}};
use std::str::FromStr;
use crate::models::{Post, Session, PasskeyCredential, AuthChallengeState};

pub type DbPool = SqlitePool;

pub async fn connect(database_url: &str) -> DbPool {
    let options = SqliteConnectOptions::from_str(database_url)
        .expect("invalid DATABASE_URL")
        .create_if_missing(true);
    SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await
        .expect("failed to connect to SQLite")
}

pub async fn run_migrations(pool: &DbPool) {
    sqlx::query(include_str!("../migrations/001_initial.sql"))
        .execute(pool)
        .await
        .expect("failed to run migrations");
}

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
    let mut tx = pool.begin().await.ok()?;

    let row = sqlx::query!("SELECT image_url FROM posts WHERE id = ?", id)
        .fetch_optional(&mut *tx)
        .await
        .ok()
        .flatten();

    if let Some(r) = row {
        sqlx::query!("DELETE FROM posts WHERE id = ?", id)
            .execute(&mut *tx)
            .await
            .ok();
        tx.commit().await.ok();
        Some(r.image_url)
    } else {
        tx.rollback().await.ok();
        None
    }
}

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
        r#"SELECT id as "id!", expires_at as "expires_at!" FROM sessions WHERE id = ? AND expires_at > datetime('now')"#,
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

pub async fn get_all_credentials(pool: &DbPool) -> Vec<PasskeyCredential> {
    sqlx::query_as!(PasskeyCredential,
        r#"SELECT id as "id!", passkey_json as "passkey_json!" FROM passkey_credentials"#
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
    let mut tx = pool.begin().await.ok()?;

    let row = sqlx::query_as!(AuthChallengeState,
        r#"SELECT id as "id!", state_json as "state_json!" FROM auth_challenge_state WHERE id = ? AND expires_at > datetime('now')"#,
        id
    )
    .fetch_optional(&mut *tx)
    .await
    .ok()
    .flatten();

    if row.is_some() {
        sqlx::query!("DELETE FROM auth_challenge_state WHERE id = ?", id)
            .execute(&mut *tx)
            .await
            .ok();
        tx.commit().await.ok();
    } else {
        tx.rollback().await.ok();
    }

    row
}

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
