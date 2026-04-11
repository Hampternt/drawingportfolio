use sqlx::{SqlitePool, sqlite::{SqlitePoolOptions, SqliteConnectOptions}};
use std::str::FromStr;
use crate::models::{Post, Session, PasskeyCredential, AuthChallengeState, FoodItem, MealEntryWithFood};

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

    // Migration 002: idempotent — errors on duplicate column are intentionally ignored
    let _ = sqlx::query(include_str!("../migrations/002_add_post_fields.sql"))
        .execute(pool)
        .await;

    // Migration 003: nutrition tracker tables
    let _ = sqlx::query(include_str!("../migrations/003_nutrition.sql"))
        .execute(pool)
        .await;

    // Migration 004: image variant URLs (webp_url, avif_url)
    let _ = sqlx::query(include_str!("../migrations/004_add_image_variants.sql"))
        .execute(pool)
        .await;

    // Migration 005: package size for food items
    let _ = sqlx::query(include_str!("../migrations/005_add_package_size.sql"))
        .execute(pool)
        .await;
}

pub async fn get_posts(pool: &DbPool, page: i64) -> Vec<Post> {
    let offset = page * 20;
    sqlx::query_as!(Post,
        "SELECT id, caption, image_url, webp_url, avif_url, format, file_size_bytes, created_at FROM posts ORDER BY created_at DESC LIMIT 21 OFFSET ?",
        offset
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default()
}

pub async fn insert_post(
    pool: &DbPool,
    caption: &str,
    image_url: &str,
    webp_url: &str,
    avif_url: &str,
    format: &str,
    file_size_bytes: i64,
) -> Post {
    let id = sqlx::query!(
        "INSERT INTO posts (caption, image_url, webp_url, avif_url, format, file_size_bytes) VALUES (?, ?, ?, ?, ?, ?) RETURNING id",
        caption, image_url, webp_url, avif_url, format, file_size_bytes
    )
    .fetch_one(pool)
    .await
    .expect("failed to insert post")
    .id;

    sqlx::query_as!(Post,
        "SELECT id, caption, image_url, webp_url, avif_url, format, file_size_bytes, created_at FROM posts WHERE id = ?", id
    )
    .fetch_one(pool)
    .await
    .expect("failed to fetch inserted post")
}

pub async fn update_post_avif_url(pool: &DbPool, id: i64, avif_url: &str) -> Result<(), sqlx::Error> {
    sqlx::query!("UPDATE posts SET avif_url = ? WHERE id = ?", avif_url, id)
        .execute(pool)
        .await?;
    Ok(())
}

pub struct PostUrls {
    pub image_url: String,
    pub webp_url: String,
    pub avif_url: String,
}

pub async fn delete_post_and_get_urls(pool: &DbPool, id: i64) -> Option<PostUrls> {
    let mut tx = pool.begin().await.ok()?;

    let row = sqlx::query!("SELECT image_url, webp_url, avif_url FROM posts WHERE id = ?", id)
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
        Some(PostUrls {
            image_url: r.image_url,
            webp_url: r.webp_url,
            avif_url: r.avif_url,
        })
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

pub async fn get_food_items(pool: &DbPool) -> Vec<FoodItem> {
    sqlx::query_as!(FoodItem,
        "SELECT id, name, brand, barcode, calories, protein, carbs, fat, fiber, sugar, sodium, saturated_fat, package_size, image_url, created_at FROM food_items ORDER BY name ASC"
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default()
}

pub async fn search_food_items(pool: &DbPool, q: &str) -> Vec<FoodItem> {
    let pattern = format!("%{}%", q);
    sqlx::query_as!(FoodItem,
        "SELECT id, name, brand, barcode, calories, protein, carbs, fat, fiber, sugar, sodium, saturated_fat, package_size, image_url, created_at FROM food_items WHERE name LIKE ? OR brand LIKE ? ORDER BY name ASC LIMIT 20",
        pattern, pattern
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default()
}

pub async fn insert_food_item(
    pool: &DbPool,
    name: &str,
    brand: &str,
    barcode: Option<&str>,
    calories: f64,
    protein: f64,
    carbs: f64,
    fat: f64,
    fiber: f64,
    sugar: f64,
    sodium: f64,
    saturated_fat: f64,
    package_size: Option<f64>,
    image_url: &str,
) -> FoodItem {
    let id = sqlx::query!(
        "INSERT INTO food_items (name, brand, barcode, calories, protein, carbs, fat, fiber, sugar, sodium, saturated_fat, package_size, image_url) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
        name, brand, barcode, calories, protein, carbs, fat, fiber, sugar, sodium, saturated_fat, package_size, image_url
    )
    .fetch_one(pool)
    .await
    .expect("failed to insert food item")
    .id;

    sqlx::query_as!(FoodItem,
        "SELECT id, name, brand, barcode, calories, protein, carbs, fat, fiber, sugar, sodium, saturated_fat, package_size, image_url, created_at FROM food_items WHERE id = ?", id
    )
    .fetch_one(pool)
    .await
    .expect("failed to fetch inserted food item")
}

pub async fn delete_food_item(pool: &DbPool, id: i64) -> Option<String> {
    let mut tx = pool.begin().await.ok()?;

    let row = sqlx::query!("SELECT image_url FROM food_items WHERE id = ?", id)
        .fetch_optional(&mut *tx)
        .await
        .ok()
        .flatten();

    if let Some(r) = row {
        sqlx::query!("DELETE FROM food_items WHERE id = ?", id)
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

pub async fn get_meal_entries_for_date(pool: &DbPool, date: &str) -> Vec<MealEntryWithFood> {
    let rows = sqlx::query!(
        r#"SELECT
            me.id as entry_id,
            fi.name as food_name,
            me.grams,
            fi.calories as base_calories,
            fi.protein as base_protein,
            fi.carbs as base_carbs,
            fi.fat as base_fat,
            fi.fiber as base_fiber,
            fi.sugar as base_sugar,
            fi.sodium as base_sodium,
            fi.saturated_fat as base_saturated_fat
        FROM meal_entries me
        JOIN food_items fi ON fi.id = me.food_item_id
        WHERE me.date = ?
        ORDER BY me.created_at ASC"#,
        date
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    rows.into_iter().map(|r| {
        let factor = r.grams / 100.0;
        MealEntryWithFood {
            entry_id: r.entry_id,
            food_name: r.food_name,
            grams: r.grams,
            calories: r.base_calories * factor,
            protein: r.base_protein * factor,
            carbs: r.base_carbs * factor,
            fat: r.base_fat * factor,
            fiber: r.base_fiber * factor,
            sugar: r.base_sugar * factor,
            sodium: r.base_sodium * factor,
            saturated_fat: r.base_saturated_fat * factor,
        }
    }).collect()
}

pub async fn insert_meal_entry(pool: &DbPool, food_item_id: i64, date: &str, grams: f64) -> Result<i64, sqlx::Error> {
    let id = sqlx::query!(
        "INSERT INTO meal_entries (food_item_id, date, grams) VALUES (?, ?, ?) RETURNING id",
        food_item_id, date, grams
    )
    .fetch_one(pool)
    .await?
    .id;
    Ok(id.ok_or_else(|| sqlx::Error::RowNotFound)?)
}

pub async fn delete_meal_entry(pool: &DbPool, id: i64) {
    sqlx::query!("DELETE FROM meal_entries WHERE id = ?", id)
        .execute(pool)
        .await
        .ok();
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
        let post = insert_post(&pool, "test caption", "https://example.com/img.jpg", "", "", crate::models::PostFormat::Single.as_str(), 0).await;
        assert_eq!(post.caption, "test caption");
        let posts = get_posts(&pool, 0).await;
        assert_eq!(posts.len(), 1);
        assert_eq!(posts[0].id, post.id);
    }

    #[tokio::test]
    async fn test_delete_post() {
        let pool = test_pool().await;
        let post = insert_post(&pool, "to delete", "https://example.com/img.jpg", "https://example.com/img-webp.webp", "https://example.com/img-avif.avif", crate::models::PostFormat::Single.as_str(), 0).await;
        let urls = delete_post_and_get_urls(&pool, post.id).await;
        assert!(urls.is_some());
        let urls = urls.unwrap();
        assert_eq!(urls.image_url, "https://example.com/img.jpg");
        assert_eq!(urls.webp_url, "https://example.com/img-webp.webp");
        assert_eq!(urls.avif_url, "https://example.com/img-avif.avif");
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
        let result = delete_post_and_get_urls(&pool, 99999).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_insert_post_stores_format_and_filesize() {
        let pool = test_pool().await;
        let fmt = crate::models::PostFormat::Single.as_str();
        let post = insert_post(&pool, "hello", "https://example.com/img.jpg", "", "", fmt, 12345).await;
        assert_eq!(post.format, "single");
        assert_eq!(post.file_size_bytes, 12345);
    }

    #[tokio::test]
    async fn test_insert_post_empty_caption() {
        let pool = test_pool().await;
        let fmt = crate::models::PostFormat::Single.as_str();
        let post = insert_post(&pool, "", "https://example.com/img.jpg", "", "", fmt, 0).await;
        assert_eq!(post.caption, "");
    }

    #[tokio::test]
    async fn test_insert_and_get_food_item() {
        let pool = test_pool().await;
        let item = insert_food_item(&pool, "Chicken Breast", "Generic", None, 165.0, 31.0, 0.0, 3.6, 0.0, 0.0, 74.0, 1.0, None, "").await;
        assert_eq!(item.name, "Chicken Breast");
        assert_eq!(item.calories, 165.0);
        assert!(item.barcode.is_none());
        let items = get_food_items(&pool).await;
        assert_eq!(items.len(), 1);
    }

    #[tokio::test]
    async fn test_search_food_items() {
        let pool = test_pool().await;
        insert_food_item(&pool, "Chicken Breast", "Generic", None, 165.0, 31.0, 0.0, 3.6, 0.0, 0.0, 74.0, 1.0, None, "").await;
        insert_food_item(&pool, "Brown Rice", "Generic", None, 112.0, 2.6, 23.5, 0.9, 1.8, 0.0, 5.0, 0.2, None, "").await;
        let results = search_food_items(&pool, "chicken").await;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Chicken Breast");
    }

    #[tokio::test]
    async fn test_delete_food_item() {
        let pool = test_pool().await;
        let item = insert_food_item(&pool, "Test Item", "", None, 100.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, None, "https://example.com/img.jpg").await;
        let url = delete_food_item(&pool, item.id).await;
        assert_eq!(url, Some("https://example.com/img.jpg".to_string()));
        assert!(get_food_items(&pool).await.is_empty());
    }

    #[tokio::test]
    async fn test_insert_meal_entry_and_get_for_date() {
        let pool = test_pool().await;
        let item = insert_food_item(&pool, "White Rice", "", None, 130.0, 2.7, 28.6, 0.3, 0.4, 0.0, 1.0, 0.1, None, "").await;
        insert_meal_entry(&pool, item.id, "2026-04-09", 200.0).await.unwrap();
        let entries = get_meal_entries_for_date(&pool, "2026-04-09").await;
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].food_name, "White Rice");
        assert_eq!(entries[0].grams, 200.0);
        assert!((entries[0].calories - 260.0).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_delete_meal_entry() {
        let pool = test_pool().await;
        let item = insert_food_item(&pool, "Apple", "", None, 52.0, 0.3, 14.0, 0.2, 2.4, 10.0, 1.0, 0.0, None, "").await;
        let entry_id = insert_meal_entry(&pool, item.id, "2026-04-09", 150.0).await.unwrap();
        delete_meal_entry(&pool, entry_id).await;
        assert!(get_meal_entries_for_date(&pool, "2026-04-09").await.is_empty());
    }

    #[tokio::test]
    async fn test_meal_entry_wrong_date_not_returned() {
        let pool = test_pool().await;
        let item = insert_food_item(&pool, "Banana", "", None, 89.0, 1.1, 23.0, 0.3, 2.6, 12.0, 1.0, 0.0, None, "").await;
        insert_meal_entry(&pool, item.id, "2026-04-08", 100.0).await.unwrap();
        let entries = get_meal_entries_for_date(&pool, "2026-04-09").await;
        assert!(entries.is_empty());
    }
}
