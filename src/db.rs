use crate::models::{
    AuthChallengeState, DrawingTaskWithImage, FoodItem, MealEntryWithFood, PasskeyCredential, Post,
    Session, Targets, TaskImage,
};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    SqlitePool,
};
use std::str::FromStr;

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

    // Migration 006: custom portion sizes for food items
    let _ = sqlx::query(include_str!("../migrations/006_add_custom_portions.sql"))
        .execute(pool)
        .await;

    // Migration 007: drawing task images and tasks
    sqlx::query(include_str!("../migrations/007_drawing_tasks.sql"))
        .execute(pool)
        .await
        .expect("failed to run drawing tasks migration");

    // Migration 008: meal slot per entry (breakfast/lunch/dinner/snack/other)
    let _ = sqlx::query(include_str!("../migrations/008_meal_slots.sql"))
        .execute(pool)
        .await;

    // Migration 009: daily nutrition targets (single row, single user)
    let _ = sqlx::query(include_str!("../migrations/009_targets.sql"))
        .execute(pool)
        .await;

    // Migration 010: food metadata — category, favourite flag, default portion
    let _ = sqlx::query(include_str!("../migrations/010_food_meta.sql"))
        .execute(pool)
        .await;

    // Migration 011: weight log + saved meals (recipes)
    let _ = sqlx::query(include_str!("../migrations/011_weights_recipes.sql"))
        .execute(pool)
        .await;

    // Migration 012: intrinsic image dimensions.
    //
    // The art feed lays cards out in a CSS multi-column masonry. Without
    // width/height on the <img>, each image reserves no height until it loads
    // and the column reflows under it. Storing the real ratio lets the browser
    // reserve the box up front.
    //
    // Existing rows keep 0 — no backfill. The originals are in object storage
    // and re-fetching them at startup would stall the boot for a layout
    // optimisation; post_card.html omits both attributes when either is 0, so
    // legacy rows render exactly as they did before.
    let _ = sqlx::query(include_str!("../migrations/012_image_dimensions.sql"))
        .execute(pool)
        .await;
}

pub async fn get_posts(pool: &DbPool, page: i64) -> Vec<Post> {
    let offset = page * 20;
    sqlx::query_as!(Post,
        "SELECT id, caption, image_url, webp_url, avif_url, format, file_size_bytes, created_at, image_width, image_height FROM posts ORDER BY created_at DESC LIMIT 21 OFFSET ?",
        offset
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default()
}

#[allow(clippy::too_many_arguments)]
pub async fn insert_post(
    pool: &DbPool,
    caption: &str,
    image_url: &str,
    webp_url: &str,
    avif_url: &str,
    format: &str,
    file_size_bytes: i64,
    image_width: i64,
    image_height: i64,
) -> Post {
    let id = sqlx::query!(
        "INSERT INTO posts (caption, image_url, webp_url, avif_url, format, file_size_bytes, image_width, image_height) VALUES (?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
        caption, image_url, webp_url, avif_url, format, file_size_bytes, image_width, image_height
    )
    .fetch_one(pool)
    .await
    .expect("failed to insert post")
    .id;

    sqlx::query_as!(Post,
        "SELECT id, caption, image_url, webp_url, avif_url, format, file_size_bytes, created_at, image_width, image_height FROM posts WHERE id = ?", id
    )
    .fetch_one(pool)
    .await
    .expect("failed to fetch inserted post")
}

pub async fn update_post_avif_url(
    pool: &DbPool,
    id: i64,
    avif_url: &str,
) -> Result<(), sqlx::Error> {
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

    let row = sqlx::query!(
        "SELECT image_url, webp_url, avif_url FROM posts WHERE id = ?",
        id
    )
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
        id,
        expires_at
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
    let challenges =
        sqlx::query!("DELETE FROM auth_challenge_state WHERE expires_at <= datetime('now')")
            .execute(pool)
            .await
            .ok();

    let session_rows = sessions.map(|r| r.rows_affected()).unwrap_or(0);
    let challenge_rows = challenges.map(|r| r.rows_affected()).unwrap_or(0);
    tracing::info!(
        "cleanup: removed {session_rows} expired sessions, {challenge_rows} expired challenges"
    );
}

pub async fn get_all_credentials(pool: &DbPool) -> Vec<PasskeyCredential> {
    sqlx::query_as!(
        PasskeyCredential,
        r#"SELECT id as "id!", passkey_json as "passkey_json!" FROM passkey_credentials"#
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default()
}

pub async fn save_credential(pool: &DbPool, id: &str, passkey_json: &str) {
    sqlx::query!(
        "INSERT OR REPLACE INTO passkey_credentials (id, passkey_json) VALUES (?, ?)",
        id,
        passkey_json
    )
    .execute(pool)
    .await
    .expect("failed to save credential");
}

pub async fn save_challenge(pool: &DbPool, id: &str, state_json: &str, expires_at: &str) {
    sqlx::query!(
        "INSERT INTO auth_challenge_state (id, state_json, expires_at) VALUES (?, ?, ?)",
        id,
        state_json,
        expires_at
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
        "SELECT id, name, brand, barcode, calories, protein, carbs, fat, fiber, sugar, sodium, saturated_fat, package_size, custom_portions, image_url, category, is_favourite, default_portion_g, created_at FROM food_items ORDER BY name ASC"
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default()
}

pub async fn search_food_items(pool: &DbPool, q: &str) -> Vec<FoodItem> {
    let pattern = format!("%{}%", q);
    sqlx::query_as!(FoodItem,
        "SELECT id, name, brand, barcode, calories, protein, carbs, fat, fiber, sugar, sodium, saturated_fat, package_size, custom_portions, image_url, category, is_favourite, default_portion_g, created_at FROM food_items WHERE name LIKE ? OR brand LIKE ? ORDER BY name ASC LIMIT 20",
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
    custom_portions: &str,
    image_url: &str,
) -> FoodItem {
    let id = sqlx::query!(
        "INSERT INTO food_items (name, brand, barcode, calories, protein, carbs, fat, fiber, sugar, sodium, saturated_fat, package_size, custom_portions, image_url) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
        name, brand, barcode, calories, protein, carbs, fat, fiber, sugar, sodium, saturated_fat, package_size, custom_portions, image_url
    )
    .fetch_one(pool)
    .await
    .expect("failed to insert food item")
    .id;

    sqlx::query_as!(FoodItem,
        "SELECT id, name, brand, barcode, calories, protein, carbs, fat, fiber, sugar, sodium, saturated_fat, package_size, custom_portions, image_url, category, is_favourite, default_portion_g, created_at FROM food_items WHERE id = ?", id
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

pub async fn get_food_item(pool: &DbPool, id: i64) -> Option<FoodItem> {
    sqlx::query_as!(FoodItem,
        "SELECT id, name, brand, barcode, calories, protein, carbs, fat, fiber, sugar, sodium, saturated_fat, package_size, custom_portions, image_url, category, is_favourite, default_portion_g, created_at FROM food_items WHERE id = ?", id
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
}

#[allow(clippy::too_many_arguments)]
pub async fn update_food_item(
    pool: &DbPool,
    id: i64,
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
    custom_portions: &str,
    image_url: &str,
    category: &str,
    is_favourite: bool,
    default_portion_g: Option<f64>,
) {
    let fav = if is_favourite { 1i64 } else { 0i64 };
    sqlx::query!(
        "UPDATE food_items SET name = ?, brand = ?, barcode = ?, calories = ?, protein = ?, carbs = ?, fat = ?, fiber = ?, sugar = ?, sodium = ?, saturated_fat = ?, package_size = ?, custom_portions = ?, image_url = ?, category = ?, is_favourite = ?, default_portion_g = ? WHERE id = ?",
        name, brand, barcode, calories, protein, carbs, fat, fiber, sugar, sodium, saturated_fat, package_size, custom_portions, image_url, category, fav, default_portion_g, id
    )
    .execute(pool)
    .await
    .ok();
}

pub async fn toggle_food_favourite(pool: &DbPool, id: i64) {
    sqlx::query!(
        "UPDATE food_items SET is_favourite = 1 - is_favourite WHERE id = ?",
        id
    )
    .execute(pool)
    .await
    .ok();
}

pub async fn get_item_log_history(
    pool: &DbPool,
    id: i64,
    start: &str,
    end: &str,
) -> Vec<(String, f64)> {
    sqlx::query!(
        r#"SELECT date as "date!", SUM(grams) as "grams!: f64" FROM meal_entries
        WHERE food_item_id = ? AND date >= ? AND date <= ?
        GROUP BY date ORDER BY date ASC"#,
        id,
        start,
        end
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|r| (r.date, r.grams))
    .collect()
}

pub async fn get_meal_entries_for_date(pool: &DbPool, date: &str) -> Vec<MealEntryWithFood> {
    let rows = sqlx::query!(
        r#"SELECT
            me.id as entry_id,
            me.food_item_id,
            fi.name as food_name,
            me.grams,
            me.slot as "slot!",
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

    rows.into_iter()
        .map(|r| {
            let factor = r.grams / 100.0;
            MealEntryWithFood {
                entry_id: r.entry_id,
                food_item_id: r.food_item_id,
                food_name: r.food_name,
                slot: r.slot,
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
        })
        .collect()
}

pub async fn insert_meal_entry(
    pool: &DbPool,
    food_item_id: i64,
    date: &str,
    grams: f64,
    slot: &str,
) -> Result<i64, sqlx::Error> {
    let id = sqlx::query!(
        "INSERT INTO meal_entries (food_item_id, date, grams, slot) VALUES (?, ?, ?, ?) RETURNING id",
        food_item_id,
        date,
        grams,
        slot
    )
    .fetch_one(pool)
    .await?
    .id;
    Ok(id.ok_or_else(|| sqlx::Error::RowNotFound)?)
}

pub async fn get_recent_foods(pool: &DbPool, limit: i64) -> Vec<crate::models::RecentFood> {
    // Bare columns + MAX() in SQLite resolve to the max row's values; the
    // annotated alias can't be referenced by name, so ORDER BY ordinal 5.
    sqlx::query!(
        r#"SELECT me.food_item_id as "food_item_id!", fi.name as "name!",
                  me.grams as "last_grams!: f64", me.slot as "last_slot!",
                  MAX(me.created_at || '-' || printf('%012d', me.id)) as "latest!: String"
        FROM meal_entries me
        JOIN food_items fi ON fi.id = me.food_item_id
        GROUP BY me.food_item_id
        ORDER BY 5 DESC
        LIMIT ?"#,
        limit
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|r| crate::models::RecentFood {
        food_item_id: r.food_item_id,
        name: r.name,
        last_grams: r.last_grams,
        last_slot: r.last_slot,
    })
    .collect()
}

pub async fn get_food_item_by_barcode(pool: &DbPool, barcode: &str) -> Option<FoodItem> {
    sqlx::query_as!(FoodItem,
        "SELECT id, name, brand, barcode, calories, protein, carbs, fat, fiber, sugar, sodium, saturated_fat, package_size, custom_portions, image_url, category, is_favourite, default_portion_g, created_at FROM food_items WHERE barcode = ?",
        barcode
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
}

pub async fn upsert_weight(pool: &DbPool, date: &str, kg: f64) {
    sqlx::query!(
        "INSERT INTO weights (date, kg) VALUES (?, ?) ON CONFLICT(date) DO UPDATE SET kg = excluded.kg",
        date,
        kg
    )
    .execute(pool)
    .await
    .ok();
}

pub async fn get_weights_since(pool: &DbPool, start: &str) -> Vec<(String, f64)> {
    sqlx::query!(
        r#"SELECT date as "date!", kg as "kg!: f64" FROM weights WHERE date >= ? ORDER BY date ASC"#,
        start
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|r| (r.date, r.kg))
    .collect()
}

pub async fn get_latest_weight(pool: &DbPool) -> Option<(String, f64)> {
    sqlx::query!(
        r#"SELECT date as "date!", kg as "kg!: f64" FROM weights ORDER BY date DESC LIMIT 1"#
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
    .map(|r| (r.date, r.kg))
}

pub async fn get_protein_by_date_range(
    pool: &DbPool,
    start: &str,
    end: &str,
) -> Vec<(String, f64)> {
    sqlx::query!(
        r#"SELECT me.date as "date!", SUM(me.grams / 100.0 * fi.protein) as "protein!: f64"
        FROM meal_entries me JOIN food_items fi ON fi.id = me.food_item_id
        WHERE me.date >= ? AND me.date <= ?
        GROUP BY me.date ORDER BY me.date ASC"#,
        start,
        end
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|r| (r.date, r.protein))
    .collect()
}

pub async fn get_logged_dates_desc(pool: &DbPool, limit: i64) -> Vec<String> {
    sqlx::query!(
        r#"SELECT DISTINCT date as "date!" FROM meal_entries ORDER BY date DESC LIMIT ?"#,
        limit
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|r| r.date)
    .collect()
}

pub async fn get_most_logged_between(
    pool: &DbPool,
    start: &str,
    end: &str,
    limit: i64,
) -> Vec<(String, i64)> {
    // ORDER BY ordinal 2: the annotated count alias can't be named in ORDER BY
    sqlx::query!(
        r#"SELECT fi.name as "name!", COUNT(*) as "n!: i64"
        FROM meal_entries me JOIN food_items fi ON fi.id = me.food_item_id
        WHERE me.date >= ? AND me.date <= ?
        GROUP BY me.food_item_id ORDER BY 2 DESC, fi.name ASC LIMIT ?"#,
        start,
        end,
        limit
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|r| (r.name, r.n))
    .collect()
}

pub async fn create_recipe_from_slot(
    pool: &DbPool,
    name: &str,
    date: &str,
    slot: &str,
) -> Option<i64> {
    let mut tx = pool.begin().await.ok()?;
    let rows = sqlx::query!(
        "SELECT food_item_id, grams FROM meal_entries WHERE date = ? AND slot = ?",
        date,
        slot
    )
    .fetch_all(&mut *tx)
    .await
    .ok()?;
    if rows.is_empty() {
        return None;
    }
    let rid = sqlx::query!("INSERT INTO recipes (name) VALUES (?) RETURNING id", name)
        .fetch_one(&mut *tx)
        .await
        .ok()?
        .id;
    for r in rows {
        sqlx::query!(
            "INSERT INTO recipe_items (recipe_id, food_item_id, grams) VALUES (?, ?, ?)",
            rid,
            r.food_item_id,
            r.grams
        )
        .execute(&mut *tx)
        .await
        .ok()?;
    }
    tx.commit().await.ok()?;
    Some(rid)
}

pub async fn get_recipes_with_totals(pool: &DbPool) -> Vec<crate::models::RecipeWithTotals> {
    sqlx::query!(
        r#"SELECT r.id as "id!", r.name as "name!",
                  COUNT(ri.id) as "item_count!: i64",
                  COALESCE(SUM(ri.grams / 100.0 * fi.calories), 0) as "total_cal!: f64"
        FROM recipes r
        LEFT JOIN recipe_items ri ON ri.recipe_id = r.id
        LEFT JOIN food_items fi ON fi.id = ri.food_item_id
        GROUP BY r.id ORDER BY r.name ASC"#
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|r| crate::models::RecipeWithTotals {
        id: r.id,
        name: r.name,
        item_count: r.item_count,
        total_cal: r.total_cal,
    })
    .collect()
}

pub async fn log_recipe(pool: &DbPool, id: i64, date: &str, slot: &str) -> u64 {
    sqlx::query!(
        "INSERT INTO meal_entries (food_item_id, date, grams, slot)
         SELECT food_item_id, ?, grams, ? FROM recipe_items WHERE recipe_id = ?",
        date,
        slot,
        id
    )
    .execute(pool)
    .await
    .map(|r| r.rows_affected())
    .unwrap_or(0)
}

pub async fn delete_recipe(pool: &DbPool, id: i64) {
    sqlx::query!("DELETE FROM recipe_items WHERE recipe_id = ?", id)
        .execute(pool)
        .await
        .ok();
    sqlx::query!("DELETE FROM recipes WHERE id = ?", id)
        .execute(pool)
        .await
        .ok();
}

pub async fn copy_day_entries(pool: &DbPool, from_date: &str, to_date: &str) -> u64 {
    sqlx::query!(
        "INSERT INTO meal_entries (food_item_id, date, grams, slot)
         SELECT food_item_id, ?, grams, slot FROM meal_entries WHERE date = ?",
        to_date,
        from_date
    )
    .execute(pool)
    .await
    .map(|r| r.rows_affected())
    .unwrap_or(0)
}

pub async fn get_calories_by_date_range(
    pool: &DbPool,
    start: &str,
    end: &str,
) -> Vec<(String, f64)> {
    sqlx::query!(
        r#"SELECT me.date as "date!", SUM(me.grams / 100.0 * fi.calories) as "cal!: f64"
        FROM meal_entries me
        JOIN food_items fi ON fi.id = me.food_item_id
        WHERE me.date >= ? AND me.date <= ?
        GROUP BY me.date
        ORDER BY me.date ASC"#,
        start,
        end
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|r| (r.date, r.cal))
    .collect()
}

pub async fn update_meal_entry(pool: &DbPool, id: i64, grams: f64, slot: &str) {
    sqlx::query!(
        "UPDATE meal_entries SET grams = ?, slot = ? WHERE id = ?",
        grams,
        slot,
        id
    )
    .execute(pool)
    .await
    .ok();
}

pub async fn get_meal_entry(pool: &DbPool, id: i64) -> Option<crate::models::MealEntry> {
    sqlx::query_as!(
        crate::models::MealEntry,
        r#"SELECT id, food_item_id, date, grams, slot as "slot!", created_at FROM meal_entries WHERE id = ?"#,
        id
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
}

pub async fn delete_meal_entry(pool: &DbPool, id: i64) {
    sqlx::query!("DELETE FROM meal_entries WHERE id = ?", id)
        .execute(pool)
        .await
        .ok();
}

pub async fn get_targets(pool: &DbPool) -> Targets {
    sqlx::query_as!(Targets,
        r#"SELECT calories as "calories!", protein as "protein!", carbs as "carbs!", fat as "fat!" FROM targets WHERE id = 1"#
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
    .unwrap_or(Targets { calories: 2400.0, protein: 165.0, carbs: 260.0, fat: 72.0 })
}

pub async fn set_targets(pool: &DbPool, calories: f64, protein: f64, carbs: f64, fat: f64) {
    sqlx::query!(
        "INSERT INTO targets (id, calories, protein, carbs, fat) VALUES (1, ?, ?, ?, ?)
         ON CONFLICT(id) DO UPDATE SET calories = excluded.calories, protein = excluded.protein,
         carbs = excluded.carbs, fat = excluded.fat",
        calories,
        protein,
        carbs,
        fat
    )
    .execute(pool)
    .await
    .ok();
}

// ── Drawing tasks ─────────────────────────────────────────────────────────────

/// Filter/sort parameters for the /tasks page. Empty string = "no filter".
/// `sort` is one of: "newest" (default), "oldest", "easiest", "hardest".
pub struct TaskFilters {
    pub subject: String,
    pub difficulty: String,
    pub task_type: String,
    pub sort: String,
}

pub async fn insert_task_image(pool: &DbPool, title: &str, image_url: &str) {
    sqlx::query!(
        "INSERT INTO task_images (title, image_url) VALUES (?, ?)",
        title,
        image_url
    )
    .execute(pool)
    .await
    .expect("failed to insert task image");
}

pub async fn get_task_images(pool: &DbPool) -> Vec<TaskImage> {
    sqlx::query_as!(TaskImage,
        "SELECT id, title, image_url, created_at FROM task_images ORDER BY created_at DESC, id DESC"
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default()
}

/// Deletes a reference image and all tasks attached to it, returning the
/// image URL so the caller can remove the object from S3.
/// Returns Some only after the transaction has committed — the caller
/// deletes the S3 object, which must never happen if the rows survived.
/// (Tasks are deleted explicitly rather than via ON DELETE CASCADE so the
/// behavior doesn't depend on the connection's foreign_keys pragma.)
pub async fn delete_task_image(pool: &DbPool, id: i64) -> Option<String> {
    let mut tx = pool.begin().await.ok()?;

    let row = sqlx::query!("SELECT image_url FROM task_images WHERE id = ?", id)
        .fetch_optional(&mut *tx)
        .await
        .ok()
        .flatten()?;

    sqlx::query!("DELETE FROM drawing_tasks WHERE image_id = ?", id)
        .execute(&mut *tx)
        .await
        .ok()?;
    sqlx::query!("DELETE FROM task_images WHERE id = ?", id)
        .execute(&mut *tx)
        .await
        .ok()?;
    tx.commit().await.ok()?;
    Some(row.image_url)
}

/// Returns false if the insert is rejected — e.g. the FK on image_id fails
/// because the reference image was deleted after the form was rendered.
pub async fn insert_drawing_task(
    pool: &DbPool,
    image_id: i64,
    title: &str,
    prompt: &str,
    subject: &str,
    difficulty: &str,
    task_type: &str,
) -> bool {
    sqlx::query!(
        "INSERT INTO drawing_tasks (image_id, title, prompt, subject, difficulty, task_type) VALUES (?, ?, ?, ?, ?, ?)",
        image_id, title, prompt, subject, difficulty, task_type
    )
    .execute(pool)
    .await
    .is_ok()
}

pub async fn delete_drawing_task(pool: &DbPool, id: i64) {
    sqlx::query!("DELETE FROM drawing_tasks WHERE id = ?", id)
        .execute(pool)
        .await
        .ok();
}

pub async fn toggle_task_completed(pool: &DbPool, id: i64) {
    sqlx::query!(
        "UPDATE drawing_tasks SET completed = 1 - completed WHERE id = ?",
        id
    )
    .execute(pool)
    .await
    .ok();
}

pub async fn get_tasks_filtered(pool: &DbPool, f: &TaskFilters) -> Vec<DrawingTaskWithImage> {
    let rows = sqlx::query!(
        r#"SELECT
            dt.id as "id!",
            dt.image_id as "image_id!",
            dt.title as "title!",
            dt.prompt as "prompt!",
            dt.subject as "subject!",
            dt.difficulty as "difficulty!",
            dt.task_type as "task_type!",
            dt.completed as "completed!",
            dt.created_at as "created_at!",
            ti.title as "image_title!",
            ti.image_url as "image_url!"
        FROM drawing_tasks dt
        JOIN task_images ti ON ti.id = dt.image_id
        WHERE (?1 = '' OR dt.subject = ?1)
          AND (?2 = '' OR dt.difficulty = ?2)
          AND (?3 = '' OR dt.task_type = ?3)
        ORDER BY
            CASE WHEN ?4 = 'easiest' THEN
                CASE dt.difficulty WHEN 'easy' THEN 0 WHEN 'medium' THEN 1 WHEN 'hard' THEN 2 ELSE 3 END
            END ASC,
            CASE WHEN ?4 = 'hardest' THEN
                CASE dt.difficulty WHEN 'hard' THEN 0 WHEN 'medium' THEN 1 WHEN 'easy' THEN 2 ELSE 3 END
            END ASC,
            CASE WHEN ?4 = 'oldest' THEN dt.id END ASC,
            dt.id DESC"#,
        f.subject, f.difficulty, f.task_type, f.sort
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    rows.into_iter()
        .map(|r| DrawingTaskWithImage {
            id: r.id,
            image_id: r.image_id,
            title: r.title,
            prompt: r.prompt,
            subject: r.subject,
            difficulty: r.difficulty,
            task_type: r.task_type,
            completed: r.completed != 0,
            created_at: r.created_at,
            image_title: r.image_title,
            image_url: r.image_url,
        })
        .collect()
}

/// Distinct non-empty subjects, for the filter dropdown and admin datalist.
pub async fn get_task_subjects(pool: &DbPool) -> Vec<String> {
    sqlx::query!(
        r#"SELECT DISTINCT subject as "subject!" FROM drawing_tasks WHERE subject != '' ORDER BY subject ASC"#
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|r| r.subject)
    .collect()
}

/// Distinct non-empty task types, for the filter dropdown and admin datalist.
pub async fn get_task_types(pool: &DbPool) -> Vec<String> {
    sqlx::query!(
        r#"SELECT DISTINCT task_type as "task_type!" FROM drawing_tasks WHERE task_type != '' ORDER BY task_type ASC"#
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|r| r.task_type)
    .collect()
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
        let post = insert_post(
            &pool,
            "test caption",
            "https://example.com/img.jpg",
            "",
            "",
            crate::models::PostFormat::Single.as_str(),
            0,
            0,
            0,
        )
        .await;
        assert_eq!(post.caption, "test caption");
        let posts = get_posts(&pool, 0).await;
        assert_eq!(posts.len(), 1);
        assert_eq!(posts[0].id, post.id);
    }

    #[tokio::test]
    async fn test_delete_post() {
        let pool = test_pool().await;
        let post = insert_post(
            &pool,
            "to delete",
            "https://example.com/img.jpg",
            "https://example.com/img-webp.webp",
            "https://example.com/img-avif.avif",
            crate::models::PostFormat::Single.as_str(),
            0,
            0,
            0,
        )
        .await;
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
        let post = insert_post(
            &pool,
            "hello",
            "https://example.com/img.jpg",
            "",
            "",
            fmt,
            12345,
            0,
            0,
        )
        .await;
        assert_eq!(post.format, "single");
        assert_eq!(post.file_size_bytes, 12345);
    }

    #[tokio::test]
    async fn test_insert_post_empty_caption() {
        let pool = test_pool().await;
        let fmt = crate::models::PostFormat::Single.as_str();
        let post = insert_post(
            &pool,
            "",
            "https://example.com/img.jpg",
            "",
            "",
            fmt,
            0,
            0,
            0,
        )
        .await;
        assert_eq!(post.caption, "");
    }

    #[tokio::test]
    async fn test_migrations_are_idempotent() {
        let pool = test_pool().await;
        // test_pool() has already run them once; a second pass must not panic.
        // SQLite has no ADD COLUMN IF NOT EXISTS, so every re-run returns a
        // duplicate-column error that the `let _ =` in run_migrations discards.
        run_migrations(&pool).await;
        assert!(get_posts(&pool, 0).await.is_empty());
    }

    #[tokio::test]
    async fn test_insert_post_persists_dimensions() {
        let pool = test_pool().await;
        let post = insert_post(
            &pool,
            "dimensioned",
            "https://example.com/img.jpg",
            "",
            "",
            crate::models::PostFormat::Single.as_str(),
            0,
            1600,
            900,
        )
        .await;
        assert_eq!(post.image_width, 1600);
        assert_eq!(post.image_height, 900);

        let fetched = &get_posts(&pool, 0).await[0];
        assert_eq!(
            fetched.image_width, 1600,
            "dimensions survive the insert/select round trip"
        );
        assert_eq!(fetched.image_height, 900);
    }

    #[tokio::test]
    async fn test_legacy_rows_read_back_as_zero_dimensions() {
        let pool = test_pool().await;
        // A pre-012 row: inserted without touching the new columns, exactly as
        // the old insert_post would have. NOT NULL DEFAULT 0 on an ALTER over an
        // existing table is worth pinning down — a NULL here would make Post's
        // i64 fail to decode at runtime rather than at compile time.
        sqlx::query(
            "INSERT INTO posts (caption, image_url, webp_url, avif_url, format, file_size_bytes) \
             VALUES ('old', 'https://example.com/old.jpg', '', '', 'single', 0)",
        )
        .execute(&pool)
        .await
        .expect("legacy-shaped insert succeeds");

        let post = &get_posts(&pool, 0).await[0];
        assert_eq!(post.image_width, 0, "legacy rows read back as 0, not NULL");
        assert_eq!(post.image_height, 0);
    }

    #[tokio::test]
    async fn test_insert_and_get_food_item() {
        let pool = test_pool().await;
        let item = insert_food_item(
            &pool,
            "Chicken Breast",
            "Generic",
            None,
            165.0,
            31.0,
            0.0,
            3.6,
            0.0,
            0.0,
            74.0,
            1.0,
            None,
            "",
            "",
        )
        .await;
        assert_eq!(item.name, "Chicken Breast");
        assert_eq!(item.calories, 165.0);
        assert!(item.barcode.is_none());
        let items = get_food_items(&pool).await;
        assert_eq!(items.len(), 1);
    }

    #[tokio::test]
    async fn test_search_food_items() {
        let pool = test_pool().await;
        insert_food_item(
            &pool,
            "Chicken Breast",
            "Generic",
            None,
            165.0,
            31.0,
            0.0,
            3.6,
            0.0,
            0.0,
            74.0,
            1.0,
            None,
            "",
            "",
        )
        .await;
        insert_food_item(
            &pool,
            "Brown Rice",
            "Generic",
            None,
            112.0,
            2.6,
            23.5,
            0.9,
            1.8,
            0.0,
            5.0,
            0.2,
            None,
            "",
            "",
        )
        .await;
        let results = search_food_items(&pool, "chicken").await;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Chicken Breast");
    }

    #[tokio::test]
    async fn test_delete_food_item() {
        let pool = test_pool().await;
        let item = insert_food_item(
            &pool,
            "Test Item",
            "",
            None,
            100.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            None,
            "",
            "https://example.com/img.jpg",
        )
        .await;
        let url = delete_food_item(&pool, item.id).await;
        assert_eq!(url, Some("https://example.com/img.jpg".to_string()));
        assert!(get_food_items(&pool).await.is_empty());
    }

    #[tokio::test]
    async fn test_insert_meal_entry_and_get_for_date() {
        let pool = test_pool().await;
        let item = insert_food_item(
            &pool,
            "White Rice",
            "",
            None,
            130.0,
            2.7,
            28.6,
            0.3,
            0.4,
            0.0,
            1.0,
            0.1,
            None,
            "",
            "",
        )
        .await;
        insert_meal_entry(&pool, item.id, "2026-04-09", 200.0, "other")
            .await
            .unwrap();
        let entries = get_meal_entries_for_date(&pool, "2026-04-09").await;
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].food_name, "White Rice");
        assert_eq!(entries[0].grams, 200.0);
        assert!((entries[0].calories - 260.0).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_delete_meal_entry() {
        let pool = test_pool().await;
        let item = insert_food_item(
            &pool, "Apple", "", None, 52.0, 0.3, 14.0, 0.2, 2.4, 10.0, 1.0, 0.0, None, "", "",
        )
        .await;
        let entry_id = insert_meal_entry(&pool, item.id, "2026-04-09", 150.0, "other")
            .await
            .unwrap();
        delete_meal_entry(&pool, entry_id).await;
        assert!(get_meal_entries_for_date(&pool, "2026-04-09")
            .await
            .is_empty());
    }

    fn no_filters() -> TaskFilters {
        TaskFilters {
            subject: String::new(),
            difficulty: String::new(),
            task_type: String::new(),
            sort: "newest".to_string(),
        }
    }

    async fn seed_image(pool: &DbPool) -> i64 {
        insert_task_image(pool, "Test model", "https://example.com/model.jpg").await;
        get_task_images(pool).await[0].id
    }

    #[tokio::test]
    async fn test_insert_task_and_get() {
        let pool = test_pool().await;
        let img_id = seed_image(&pool).await;
        insert_drawing_task(
            &pool,
            img_id,
            "Draw the hands",
            "Focus on the hands only",
            "anatomy",
            "hard",
            "focus study",
        )
        .await;
        let tasks = get_tasks_filtered(&pool, &no_filters()).await;
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].title, "Draw the hands");
        assert_eq!(tasks[0].image_url, "https://example.com/model.jpg");
        assert_eq!(tasks[0].image_title, "Test model");
        assert!(!tasks[0].completed);
    }

    #[tokio::test]
    async fn test_multiple_tasks_on_one_image() {
        let pool = test_pool().await;
        let img_id = seed_image(&pool).await;
        insert_drawing_task(
            &pool,
            img_id,
            "Focus on hands",
            "",
            "anatomy",
            "hard",
            "focus study",
        )
        .await;
        insert_drawing_task(
            &pool,
            img_id,
            "Redraw in ink style",
            "",
            "style",
            "medium",
            "style study",
        )
        .await;
        insert_drawing_task(
            &pool,
            img_id,
            "Change the lighting",
            "",
            "lighting",
            "easy",
            "modification",
        )
        .await;
        let tasks = get_tasks_filtered(&pool, &no_filters()).await;
        assert_eq!(tasks.len(), 3);
        assert!(tasks.iter().all(|t| t.image_id == img_id));
    }

    #[tokio::test]
    async fn test_task_filters() {
        let pool = test_pool().await;
        let img_id = seed_image(&pool).await;
        insert_drawing_task(&pool, img_id, "A", "", "anatomy", "hard", "focus study").await;
        insert_drawing_task(&pool, img_id, "B", "", "style", "easy", "style study").await;

        let mut f = no_filters();
        f.subject = "anatomy".to_string();
        let tasks = get_tasks_filtered(&pool, &f).await;
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].title, "A");

        let mut f = no_filters();
        f.difficulty = "easy".to_string();
        let tasks = get_tasks_filtered(&pool, &f).await;
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].title, "B");

        let mut f = no_filters();
        f.task_type = "style study".to_string();
        let tasks = get_tasks_filtered(&pool, &f).await;
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].title, "B");
    }

    #[tokio::test]
    async fn test_task_sort_by_difficulty() {
        let pool = test_pool().await;
        let img_id = seed_image(&pool).await;
        insert_drawing_task(&pool, img_id, "hard one", "", "", "hard", "").await;
        insert_drawing_task(&pool, img_id, "easy one", "", "", "easy", "").await;
        insert_drawing_task(&pool, img_id, "medium one", "", "", "medium", "").await;

        let mut f = no_filters();
        f.sort = "easiest".to_string();
        let tasks = get_tasks_filtered(&pool, &f).await;
        let difficulties: Vec<&str> = tasks.iter().map(|t| t.difficulty.as_str()).collect();
        assert_eq!(difficulties, vec!["easy", "medium", "hard"]);

        f.sort = "hardest".to_string();
        let tasks = get_tasks_filtered(&pool, &f).await;
        let difficulties: Vec<&str> = tasks.iter().map(|t| t.difficulty.as_str()).collect();
        assert_eq!(difficulties, vec!["hard", "medium", "easy"]);
    }

    #[tokio::test]
    async fn test_toggle_task_completed() {
        let pool = test_pool().await;
        let img_id = seed_image(&pool).await;
        insert_drawing_task(&pool, img_id, "toggle me", "", "", "medium", "").await;
        let id = get_tasks_filtered(&pool, &no_filters()).await[0].id;

        toggle_task_completed(&pool, id).await;
        assert!(get_tasks_filtered(&pool, &no_filters()).await[0].completed);
        toggle_task_completed(&pool, id).await;
        assert!(!get_tasks_filtered(&pool, &no_filters()).await[0].completed);
    }

    #[tokio::test]
    async fn test_insert_task_with_missing_image_fails_cleanly() {
        let pool = test_pool().await;
        // No task_images row with id 999 — the FK (enforced: sqlx enables
        // PRAGMA foreign_keys by default) must reject this without panicking.
        assert!(!insert_drawing_task(&pool, 999, "orphan", "", "", "medium", "").await);
        assert!(get_tasks_filtered(&pool, &no_filters()).await.is_empty());
    }

    #[tokio::test]
    async fn test_delete_task_image_removes_tasks() {
        let pool = test_pool().await;
        let img_id = seed_image(&pool).await;
        insert_drawing_task(&pool, img_id, "orphan-to-be", "", "", "medium", "").await;

        let url = delete_task_image(&pool, img_id).await;
        assert_eq!(url, Some("https://example.com/model.jpg".to_string()));
        assert!(get_task_images(&pool).await.is_empty());
        assert!(get_tasks_filtered(&pool, &no_filters()).await.is_empty());
    }

    #[tokio::test]
    async fn test_delete_drawing_task() {
        let pool = test_pool().await;
        let img_id = seed_image(&pool).await;
        insert_drawing_task(&pool, img_id, "delete me", "", "", "medium", "").await;
        let id = get_tasks_filtered(&pool, &no_filters()).await[0].id;
        delete_drawing_task(&pool, id).await;
        assert!(get_tasks_filtered(&pool, &no_filters()).await.is_empty());
        // image stays
        assert_eq!(get_task_images(&pool).await.len(), 1);
    }

    #[tokio::test]
    async fn test_task_filter_options() {
        let pool = test_pool().await;
        let img_id = seed_image(&pool).await;
        insert_drawing_task(&pool, img_id, "A", "", "anatomy", "hard", "focus study").await;
        insert_drawing_task(&pool, img_id, "B", "", "style", "easy", "style study").await;
        insert_drawing_task(&pool, img_id, "C", "", "", "easy", "").await;

        assert_eq!(get_task_subjects(&pool).await, vec!["anatomy", "style"]);
        assert_eq!(
            get_task_types(&pool).await,
            vec!["focus study", "style study"]
        );
    }

    #[tokio::test]
    async fn test_targets_default_and_set() {
        let pool = test_pool().await;
        let t = get_targets(&pool).await;
        assert_eq!(t.calories, 2400.0);
        assert_eq!(t.protein, 165.0);
        set_targets(&pool, 2200.0, 170.0, 240.0, 70.0).await;
        let t = get_targets(&pool).await;
        assert_eq!(t.calories, 2200.0);
        assert_eq!(t.fat, 70.0);
    }

    #[tokio::test]
    async fn test_favourite_and_category_roundtrip() {
        let pool = test_pool().await;
        let item = insert_food_item(
            &pool,
            "Skyr",
            "Arla",
            None,
            63.0,
            11.0,
            4.0,
            0.2,
            0.0,
            4.0,
            45.0,
            0.1,
            Some(450.0),
            "",
            "",
        )
        .await;
        assert_eq!(item.category, "");
        assert_eq!(item.is_favourite, 0);
        toggle_food_favourite(&pool, item.id).await;
        assert_eq!(get_food_item(&pool, item.id).await.unwrap().is_favourite, 1);
        update_food_item(
            &pool,
            item.id,
            "Skyr",
            "Arla",
            None,
            63.0,
            11.0,
            4.0,
            0.2,
            0.0,
            4.0,
            45.0,
            0.1,
            Some(450.0),
            "",
            "",
            "Dairy & eggs",
            true,
            Some(170.0),
        )
        .await;
        let item = get_food_item(&pool, item.id).await.unwrap();
        assert_eq!(item.category, "Dairy & eggs");
        assert_eq!(item.is_favourite, 1);
        assert_eq!(item.default_portion_g, Some(170.0));
        toggle_food_favourite(&pool, item.id).await;
        assert_eq!(get_food_item(&pool, item.id).await.unwrap().is_favourite, 0);
    }

    #[tokio::test]
    async fn test_item_log_history() {
        let pool = test_pool().await;
        let item = insert_food_item(
            &pool, "Oats", "", None, 379.0, 13.0, 60.0, 6.5, 0.0, 0.0, 0.0, 0.0, None, "", "",
        )
        .await;
        insert_meal_entry(&pool, item.id, "2026-07-30", 80.0, "breakfast")
            .await
            .unwrap();
        insert_meal_entry(&pool, item.id, "2026-07-30", 40.0, "snack")
            .await
            .unwrap();
        insert_meal_entry(&pool, item.id, "2026-07-31", 80.0, "breakfast")
            .await
            .unwrap();
        let hist = get_item_log_history(&pool, item.id, "2026-07-18", "2026-07-31").await;
        assert_eq!(hist.len(), 2);
        assert_eq!(hist[0], ("2026-07-30".to_string(), 120.0));
    }

    #[tokio::test]
    async fn test_recent_foods_dedup_and_order() {
        let pool = test_pool().await;
        let a = insert_food_item(
            &pool, "Skyr", "", None, 63.0, 11.0, 4.0, 0.2, 0.0, 0.0, 0.0, 0.0, None, "", "",
        )
        .await;
        let b = insert_food_item(
            &pool, "Oats", "", None, 379.0, 13.2, 60.1, 6.5, 0.0, 0.0, 0.0, 0.0, None, "", "",
        )
        .await;
        insert_meal_entry(&pool, a.id, "2026-07-30", 250.0, "breakfast")
            .await
            .unwrap();
        insert_meal_entry(&pool, b.id, "2026-07-31", 80.0, "breakfast")
            .await
            .unwrap();
        insert_meal_entry(&pool, a.id, "2026-08-01", 300.0, "snack")
            .await
            .unwrap();
        let recent = get_recent_foods(&pool, 8).await;
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].name, "Skyr"); // most recently logged first
        assert_eq!(recent[0].last_grams, 300.0); // grams of the latest log
        assert_eq!(recent[0].last_slot, "snack");
    }

    #[tokio::test]
    async fn test_get_food_item_by_barcode() {
        let pool = test_pool().await;
        insert_food_item(
            &pool,
            "Bar",
            "Barebells",
            Some("5060123456789"),
            200.0,
            20.0,
            16.0,
            8.0,
            0.0,
            0.0,
            0.0,
            0.0,
            Some(55.0),
            "",
            "",
        )
        .await;
        assert!(get_food_item_by_barcode(&pool, "5060123456789")
            .await
            .is_some());
        assert!(get_food_item_by_barcode(&pool, "0000000000000")
            .await
            .is_none());
    }

    #[tokio::test]
    async fn test_weight_upsert_and_range() {
        let pool = test_pool().await;
        upsert_weight(&pool, "2026-07-30", 82.7).await;
        upsert_weight(&pool, "2026-07-31", 82.4).await;
        upsert_weight(&pool, "2026-07-31", 82.5).await; // same-day overwrite
        let all = get_weights_since(&pool, "2026-07-01").await;
        assert_eq!(all.len(), 2);
        assert_eq!(all[1], ("2026-07-31".to_string(), 82.5));
        assert_eq!(
            get_latest_weight(&pool).await,
            Some(("2026-07-31".to_string(), 82.5))
        );
    }

    #[tokio::test]
    async fn test_recipe_create_and_log() {
        let pool = test_pool().await;
        let a = insert_food_item(
            &pool, "Oats", "", None, 379.0, 13.0, 60.0, 6.5, 0.0, 0.0, 0.0, 0.0, None, "", "",
        )
        .await;
        let b = insert_food_item(
            &pool, "Skyr", "", None, 63.0, 11.0, 4.0, 0.2, 0.0, 0.0, 0.0, 0.0, None, "", "",
        )
        .await;
        insert_meal_entry(&pool, a.id, "2026-07-31", 80.0, "breakfast")
            .await
            .unwrap();
        insert_meal_entry(&pool, b.id, "2026-07-31", 250.0, "breakfast")
            .await
            .unwrap();
        assert!(
            create_recipe_from_slot(&pool, "Overnight oats", "2026-07-31", "dinner")
                .await
                .is_none()
        ); // empty slot
        let rid = create_recipe_from_slot(&pool, "Overnight oats", "2026-07-31", "breakfast")
            .await
            .unwrap();
        let recipes = get_recipes_with_totals(&pool).await;
        assert_eq!(recipes.len(), 1);
        assert_eq!(recipes[0].item_count, 2);
        assert!((recipes[0].total_cal - (379.0 * 0.8 + 63.0 * 2.5)).abs() < 0.1);
        let inserted = log_recipe(&pool, rid, "2026-08-01", "snack").await;
        assert_eq!(inserted, 2);
        let entries = get_meal_entries_for_date(&pool, "2026-08-01").await;
        assert!(entries.iter().all(|e| e.slot == "snack"));
        delete_recipe(&pool, rid).await;
        assert!(get_recipes_with_totals(&pool).await.is_empty());
    }

    #[tokio::test]
    async fn test_protein_range_and_logged_dates() {
        let pool = test_pool().await;
        let a = insert_food_item(
            &pool, "Chicken", "", None, 165.0, 31.0, 0.0, 3.6, 0.0, 0.0, 0.0, 0.0, None, "", "",
        )
        .await;
        insert_meal_entry(&pool, a.id, "2026-07-30", 200.0, "lunch")
            .await
            .unwrap();
        insert_meal_entry(&pool, a.id, "2026-07-31", 100.0, "lunch")
            .await
            .unwrap();
        let prot = get_protein_by_date_range(&pool, "2026-07-30", "2026-07-31").await;
        assert_eq!(prot.len(), 2);
        assert!((prot[0].1 - 62.0).abs() < 0.01);
        assert_eq!(
            get_logged_dates_desc(&pool, 10).await,
            vec!["2026-07-31", "2026-07-30"]
        );
        let most = get_most_logged_between(&pool, "2026-07-27", "2026-08-02", 5).await;
        assert_eq!(most[0], ("Chicken".to_string(), 2));
    }

    #[tokio::test]
    async fn test_copy_day_entries() {
        let pool = test_pool().await;
        let item = insert_food_item(
            &pool, "Oats", "", None, 379.0, 13.2, 60.1, 6.5, 0.0, 0.0, 0.0, 0.0, None, "", "",
        )
        .await;
        insert_meal_entry(&pool, item.id, "2026-07-31", 80.0, "breakfast")
            .await
            .unwrap();
        insert_meal_entry(&pool, item.id, "2026-07-31", 120.0, "lunch")
            .await
            .unwrap();
        let copied = copy_day_entries(&pool, "2026-07-31", "2026-08-01").await;
        assert_eq!(copied, 2);
        let entries = get_meal_entries_for_date(&pool, "2026-08-01").await;
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].slot, "breakfast");
        assert_eq!(entries[1].grams, 120.0);
    }

    #[tokio::test]
    async fn test_calories_by_date_range() {
        let pool = test_pool().await;
        let item = insert_food_item(
            &pool, "Rice", "", None, 100.0, 2.0, 20.0, 1.0, 0.0, 0.0, 0.0, 0.0, None, "", "",
        )
        .await;
        insert_meal_entry(&pool, item.id, "2026-07-27", 100.0, "lunch")
            .await
            .unwrap();
        insert_meal_entry(&pool, item.id, "2026-07-27", 50.0, "dinner")
            .await
            .unwrap();
        insert_meal_entry(&pool, item.id, "2026-07-29", 200.0, "lunch")
            .await
            .unwrap();
        insert_meal_entry(&pool, item.id, "2026-08-05", 100.0, "lunch")
            .await
            .unwrap(); // outside range
        let rows = get_calories_by_date_range(&pool, "2026-07-26", "2026-08-01").await;
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].0, "2026-07-27");
        assert!((rows[0].1 - 150.0).abs() < 0.01);
        assert!((rows[1].1 - 200.0).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_meal_entry_slot_roundtrip() {
        let pool = test_pool().await;
        let item = insert_food_item(
            &pool, "Skyr", "", None, 63.0, 11.0, 4.0, 0.2, 0.0, 4.0, 45.0, 0.1, None, "", "",
        )
        .await;
        let id = insert_meal_entry(&pool, item.id, "2026-08-01", 250.0, "breakfast")
            .await
            .unwrap();
        let entries = get_meal_entries_for_date(&pool, "2026-08-01").await;
        assert_eq!(entries[0].slot, "breakfast");
        assert_eq!(entries[0].food_item_id, item.id);
        update_meal_entry(&pool, id, 300.0, "lunch").await;
        let entries = get_meal_entries_for_date(&pool, "2026-08-01").await;
        assert_eq!(entries[0].grams, 300.0);
        assert_eq!(entries[0].slot, "lunch");
        let raw = get_meal_entry(&pool, id).await.unwrap();
        assert_eq!(raw.slot, "lunch");
    }

    #[tokio::test]
    async fn test_meal_entry_wrong_date_not_returned() {
        let pool = test_pool().await;
        let item = insert_food_item(
            &pool, "Banana", "", None, 89.0, 1.1, 23.0, 0.3, 2.6, 12.0, 1.0, 0.0, None, "", "",
        )
        .await;
        insert_meal_entry(&pool, item.id, "2026-04-08", 100.0, "other")
            .await
            .unwrap();
        let entries = get_meal_entries_for_date(&pool, "2026-04-09").await;
        assert!(entries.is_empty());
    }
}
