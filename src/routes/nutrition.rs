use axum::{
    Router,
    routing::{get, post, put, delete},
    response::{Html, IntoResponse},
    extract::{State, Path, Query, Multipart},
    http::StatusCode,
};
use askama::Template;
use std::sync::Arc;
use std::collections::HashMap;
use crate::{AppState, middleware::{OptionalAuth, AuthSession}};

// ── HTML helpers ──────────────────────────────────────────────────────────────

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
     .replace('<', "&lt;")
     .replace('>', "&gt;")
     .replace('"', "&quot;")
}

fn fmt_nutrient(v: f64) -> String {
    if v == 0.0 { "0".to_string() } else { format!("{:.1}", v) }
}

pub fn food_item_card_html(item: &crate::models::FoodItem, is_admin: bool) -> String {
    let img_html = if item.image_url.is_empty() {
        String::new()
    } else {
        format!("<img src=\"{}\" alt=\"{}\" class=\"food-thumb\" loading=\"lazy\">",
            html_escape(&item.image_url), html_escape(&item.name))
    };
    let brand_html = if item.brand.is_empty() {
        String::new()
    } else {
        format!("<span class=\"food-brand\">{}</span>", html_escape(&item.brand))
    };
    let pkg_html = if let Some(pkg) = item.package_size {
        format!("<span class=\"food-pkg\">{}g pkg</span>", fmt_nutrient(pkg))
    } else {
        String::new()
    };
    let admin_btns = if is_admin {
        format!(
            "<div class=\"food-admin-btns\">\
             <button class=\"food-edit-btn\" hx-get=\"/api/nutrition/food-items/{}/edit\" \
             hx-target=\"#food-item-{}\" hx-swap=\"outerHTML\">Edit</button>\
             <button class=\"food-delete-btn\" hx-delete=\"/api/nutrition/food-items/{}\" \
             hx-target=\"#food-library\" hx-swap=\"innerHTML\" \
             hx-confirm=\"Delete this food item?\">×</button></div>",
            item.id, item.id, item.id
        )
    } else {
        String::new()
    };
    format!(
        r#"<li class="food-item-card" id="food-item-{}">
  {}
  <div class="food-info">
    <strong>{}</strong> {}
    <span class="food-macros">{} cal · P {}g · C {}g · F {}g{}</span>
  </div>
  {}
</li>"#,
        item.id, img_html,
        html_escape(&item.name), brand_html,
        fmt_nutrient(item.calories), fmt_nutrient(item.protein),
        fmt_nutrient(item.carbs), fmt_nutrient(item.fat),
        pkg_html,
        admin_btns
    )
}

pub fn meal_entry_row_html(entry: &crate::models::MealEntryWithFood, date: &str, is_admin: bool) -> String {
    let delete_btn = if is_admin {
        format!(
            "<button class=\"food-delete-btn\" hx-delete=\"/api/nutrition/entries/{}?date={}\" \
             hx-target=\"#day-section\" hx-swap=\"innerHTML\">×</button>",
            entry.entry_id, html_escape(date)
        )
    } else {
        String::new()
    };
    format!(
        r#"<li class="meal-entry" id="entry-{}">
  <span class="entry-name">{}</span>
  <span class="entry-grams">{}g</span>
  <span class="entry-cal">{} cal</span>
  {}
</li>"#,
        entry.entry_id,
        html_escape(&entry.food_name),
        fmt_nutrient(entry.grams),
        fmt_nutrient(entry.calories),
        delete_btn
    )
}

pub fn day_section_html(entries: &[crate::models::MealEntryWithFood], date: &str, food_items: &[crate::models::FoodItem], is_admin: bool) -> String {
    let total_cal: f64 = entries.iter().map(|e| e.calories).sum();
    let total_protein: f64 = entries.iter().map(|e| e.protein).sum();
    let total_carbs: f64 = entries.iter().map(|e| e.carbs).sum();
    let total_fat: f64 = entries.iter().map(|e| e.fat).sum();

    let entries_html: String = entries.iter()
        .map(|e| meal_entry_row_html(e, date, is_admin))
        .collect::<Vec<_>>()
        .join("\n");

    let options_html: String = food_items.iter()
        .map(|fi| {
            let pkg_attr = if let Some(pkg) = fi.package_size {
                format!(" data-package-size=\"{}\"", pkg)
            } else {
                String::new()
            };
            let cp_attr = if fi.custom_portions.is_empty() {
                String::new()
            } else {
                format!(" data-custom-portions=\"{}\"", html_escape(&fi.custom_portions))
            };
            format!("<option value=\"{}\"{}{}>{} {}</option>",
                fi.id,
                pkg_attr,
                cp_attr,
                html_escape(&fi.name),
                if fi.brand.is_empty() { String::new() } else { format!("({})", html_escape(&fi.brand)) }
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r##"<div class="day-totals">
  <span class="total-cal">{} cal</span>
  <span class="total-macro">P {}g</span>
  <span class="total-macro">C {}g</span>
  <span class="total-macro">F {}g</span>
</div>
<ul class="meal-list">
{}</ul>
<form class="log-entry-form"
      hx-post="/api/nutrition/entries"
      hx-target="#day-section"
      hx-swap="innerHTML"
      hx-on::after-request="this.reset(); onFoodSelect(this.querySelector('[name=food_item_id]'))">
  <input type="hidden" name="date" value="{}">
  <select name="food_item_id" required onchange="onFoodSelect(this)">
    <option value="">— pick food —</option>
{}
  </select>
  <select name="portion" class="portion-select" onchange="onPortionChange(this)" disabled>
    <option value="custom">Custom</option>
    <option value="1">Full</option>
    <option value="0.5">Half</option>
    <option value="0.25">Quarter</option>
    <option value="0.125">Eighth</option>
  </select>
  <input type="number" name="grams" value="100" min="1" max="5000" step="0.1" required>
  <span class="grams-label">g</span>
  <button type="submit" class="btn-primary">Log</button>
</form>"##,
        fmt_nutrient(total_cal), fmt_nutrient(total_protein),
        fmt_nutrient(total_carbs), fmt_nutrient(total_fat),
        entries_html,
        html_escape(date),
        options_html
    )
}

pub fn library_list_html(items: &[crate::models::FoodItem], is_admin: bool) -> String {
    let cards: String = items.iter()
        .map(|i| food_item_card_html(i, is_admin))
        .collect::<Vec<_>>()
        .join("\n");
    format!("<ul class=\"food-library-list\">\n{}\n</ul>", cards)
}

fn edit_food_form_html(item: &crate::models::FoodItem) -> String {
    let barcode_val = item.barcode.as_deref().unwrap_or("");
    let pkg_val = item.package_size.map(|p| fmt_nutrient(p)).unwrap_or_default();
    format!(
        r##"<li class="food-item-card editing" id="food-item-{id}">
<form class="nutrient-form edit-food-form"
      hx-put="/api/nutrition/food-items/{id}"
      hx-target="#food-library"
      hx-swap="innerHTML"
      hx-encoding="multipart/form-data">
  <input type="text" name="name" value="{name}" placeholder="Name *" required>
  <input type="text" name="brand" value="{brand}" placeholder="Brand">
  <input type="text" name="barcode" value="{barcode}" placeholder="Barcode">
  <div class="nutrient-grid">
    <label>Calories/100g<input type="number" name="calories" step="0.1" min="0" value="{calories}"></label>
    <label>Protein/100g<input type="number" name="protein" step="0.1" min="0" value="{protein}"></label>
    <label>Carbs/100g<input type="number" name="carbs" step="0.1" min="0" value="{carbs}"></label>
    <label>Fat/100g<input type="number" name="fat" step="0.1" min="0" value="{fat}"></label>
    <label>Fiber/100g<input type="number" name="fiber" step="0.1" min="0" value="{fiber}"></label>
    <label>Sugar/100g<input type="number" name="sugar" step="0.1" min="0" value="{sugar}"></label>
    <label>Sodium/100g (mg)<input type="number" name="sodium" step="0.1" min="0" value="{sodium}"></label>
    <label>Sat. fat/100g<input type="number" name="saturated_fat" step="0.1" min="0" value="{sat_fat}"></label>
  </div>
  <label class="package-size-label">Package / total size (g)<input type="number" name="package_size" step="0.1" min="0" value="{pkg}" placeholder="e.g. 565"></label>
  <label class="package-size-label">Custom portions (g, comma-separated)<input type="text" name="custom_portions" value="{custom_portions}" placeholder="e.g. 125, 250, 375"></label>
  <label class="file-label">Image <input type="file" name="image" accept="image/jpeg,image/png,image/webp"></label>
  <input type="hidden" name="image_url" value="{image_url}">
  <div class="form-actions">
    <button type="submit" class="btn-primary">Save</button>
    <button type="button" class="btn-secondary"
            hx-get="/api/nutrition/food-items/{id}/card"
            hx-target="#food-item-{id}"
            hx-swap="outerHTML">Cancel</button>
  </div>
</form>
</li>"##,
        id = item.id,
        name = html_escape(&item.name),
        brand = html_escape(&item.brand),
        barcode = html_escape(barcode_val),
        calories = fmt_nutrient(item.calories),
        protein = fmt_nutrient(item.protein),
        carbs = fmt_nutrient(item.carbs),
        fat = fmt_nutrient(item.fat),
        fiber = fmt_nutrient(item.fiber),
        sugar = fmt_nutrient(item.sugar),
        sodium = fmt_nutrient(item.sodium),
        sat_fat = fmt_nutrient(item.saturated_fat),
        pkg = pkg_val,
        custom_portions = html_escape(&item.custom_portions),
        image_url = html_escape(&item.image_url),
    )
}

// ── Askama template ───────────────────────────────────────────────────────────

#[derive(Template)]
#[template(path = "fitness/feed.html")]
struct FitnessTemplate {
    is_admin: bool,
    today: String,
    day_section_html: String,
    library_html: String,
}

// ── Route handlers ────────────────────────────────────────────────────────────

async fn fitness_page(
    OptionalAuth(is_admin): OptionalAuth,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let entries = crate::db::get_meal_entries_for_date(&state.pool, &today).await;
    let food_items = crate::db::get_food_items(&state.pool).await;
    let day_html = day_section_html(&entries, &today, &food_items, is_admin);
    let lib_html = library_list_html(&food_items, is_admin);
    Html(FitnessTemplate {
        is_admin,
        today,
        day_section_html: day_html,
        library_html: lib_html,
    }.render().unwrap())
}

async fn htmx_day(
    OptionalAuth(is_admin): OptionalAuth,
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let date = params.get("date").cloned().unwrap_or_else(|| {
        chrono::Utc::now().format("%Y-%m-%d").to_string()
    });
    let entries = crate::db::get_meal_entries_for_date(&state.pool, &date).await;
    let food_items = crate::db::get_food_items(&state.pool).await;
    Html(day_section_html(&entries, &date, &food_items, is_admin))
}

async fn add_food_item(
    OptionalAuth(is_admin): OptionalAuth,
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let mut name = String::new();
    let mut brand = String::new();
    let mut barcode: Option<String> = None;
    let mut calories = 0f64;
    let mut protein = 0f64;
    let mut carbs = 0f64;
    let mut fat = 0f64;
    let mut fiber = 0f64;
    let mut sugar = 0f64;
    let mut sodium = 0f64;
    let mut saturated_fat = 0f64;
    let mut package_size: Option<f64> = None;
    let mut custom_portions = String::new();
    let mut image_url = String::new();
    let mut image_bytes: Option<Vec<u8>> = None;

    while let Ok(Some(field)) = multipart.next_field().await {
        match field.name() {
            Some("name") => name = field.text().await.unwrap_or_default().trim().to_string(),
            Some("brand") => brand = field.text().await.unwrap_or_default().trim().to_string(),
            Some("barcode") => {
                let v = field.text().await.unwrap_or_default();
                let v = v.trim();
                if !v.is_empty() { barcode = Some(v.to_string()); }
            }
            Some("calories") => calories = field.text().await.unwrap_or_default().trim().parse().unwrap_or(0.0),
            Some("protein") => protein = field.text().await.unwrap_or_default().trim().parse().unwrap_or(0.0),
            Some("carbs") => carbs = field.text().await.unwrap_or_default().trim().parse().unwrap_or(0.0),
            Some("fat") => fat = field.text().await.unwrap_or_default().trim().parse().unwrap_or(0.0),
            Some("fiber") => fiber = field.text().await.unwrap_or_default().trim().parse().unwrap_or(0.0),
            Some("sugar") => sugar = field.text().await.unwrap_or_default().trim().parse().unwrap_or(0.0),
            Some("sodium") => sodium = field.text().await.unwrap_or_default().trim().parse().unwrap_or(0.0),
            Some("saturated_fat") => saturated_fat = field.text().await.unwrap_or_default().trim().parse().unwrap_or(0.0),
            Some("package_size") => {
                let v: f64 = field.text().await.unwrap_or_default().trim().parse().unwrap_or(0.0);
                if v > 0.0 { package_size = Some(v); }
            }
            Some("custom_portions") => custom_portions = field.text().await.unwrap_or_default().trim().to_string(),
            Some("image_url") => image_url = field.text().await.unwrap_or_default().trim().to_string(),
            Some("image") => {
                let bytes = field.bytes().await.unwrap_or_default();
                if !bytes.is_empty() {
                    image_bytes = Some(bytes.to_vec());
                }
            }
            _ => {}
        }
    }

    if name.is_empty() {
        return (StatusCode::BAD_REQUEST, Html("<p>Name is required</p>".to_string())).into_response();
    }

    // Upload image to S3 if provided and user is admin
    let mut uploaded_to_s3 = false;
    if is_admin {
        if let Some(bytes) = image_bytes {
            if let Some(ext) = crate::routes::admin::validate_magic_bytes(&bytes) {
                let ct = format!("image/{ext}");
                let key = format!("food/{}.{}", uuid::Uuid::new_v4(), ext);
                if let Ok(url) = state.storage.upload(&key, bytes, &ct).await {
                    image_url = url;
                    uploaded_to_s3 = true;
                }
            }
        }
    }

    // Only allow OpenFoodFacts CDN URLs, our own S3 uploads, or empty
    if !image_url.is_empty()
        && !uploaded_to_s3
        && !image_url.starts_with("https://images.openfoodfacts.org/")
        && !image_url.starts_with("https://static.openfoodfacts.org/")
        && !image_url.starts_with("https://world.openfoodfacts.org/")
    {
        image_url = String::new();
    }

    let _item = crate::db::insert_food_item(
        &state.pool, &name, &brand, barcode.as_deref(),
        calories, protein, carbs, fat, fiber, sugar, sodium, saturated_fat, package_size, &custom_portions, &image_url,
    ).await;

    let all_items = crate::db::get_food_items(&state.pool).await;
    Html(library_list_html(&all_items, is_admin)).into_response()
}

async fn delete_food_item_handler(
    AuthSession(_): AuthSession,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    if let Some(img_url) = crate::db::delete_food_item(&state.pool, id).await {
        if !img_url.is_empty() {
            let _ = state.storage.delete_by_url(&img_url).await;
        }
    }
    let items = crate::db::get_food_items(&state.pool).await;
    Html(library_list_html(&items, true))
}

async fn add_meal_entry(
    OptionalAuth(is_admin): OptionalAuth,
    State(state): State<Arc<AppState>>,
    axum::Form(form): axum::Form<HashMap<String, String>>,
) -> impl IntoResponse {
    let date = form.get("date").cloned().unwrap_or_else(|| {
        chrono::Utc::now().format("%Y-%m-%d").to_string()
    });
    let food_item_id: i64 = form.get("food_item_id")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let grams: f64 = form.get("grams")
        .and_then(|v| v.parse().ok())
        .unwrap_or(100.0);

    if food_item_id == 0 || grams <= 0.0 {
        let entries = crate::db::get_meal_entries_for_date(&state.pool, &date).await;
        let food_items = crate::db::get_food_items(&state.pool).await;
        return Html(day_section_html(&entries, &date, &food_items, is_admin)).into_response();
    }

    let _ = crate::db::insert_meal_entry(&state.pool, food_item_id, &date, grams).await;
    let entries = crate::db::get_meal_entries_for_date(&state.pool, &date).await;
    let food_items = crate::db::get_food_items(&state.pool).await;
    Html(day_section_html(&entries, &date, &food_items, is_admin)).into_response()
}

async fn delete_meal_entry_handler(
    AuthSession(_): AuthSession,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    crate::db::delete_meal_entry(&state.pool, id).await;
    let date = params.get("date").cloned().unwrap_or_else(|| {
        chrono::Utc::now().format("%Y-%m-%d").to_string()
    });
    let entries = crate::db::get_meal_entries_for_date(&state.pool, &date).await;
    let food_items = crate::db::get_food_items(&state.pool).await;
    Html(day_section_html(&entries, &date, &food_items, true))
}

async fn edit_food_form(
    AuthSession(_): AuthSession,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    match crate::db::get_food_item(&state.pool, id).await {
        Some(item) => Html(edit_food_form_html(&item)).into_response(),
        None => (StatusCode::NOT_FOUND, Html("<p>Food item not found</p>".to_string())).into_response(),
    }
}

async fn food_item_card(
    AuthSession(_): AuthSession,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    match crate::db::get_food_item(&state.pool, id).await {
        Some(item) => Html(food_item_card_html(&item, true)).into_response(),
        None => (StatusCode::NOT_FOUND, Html("<p>Food item not found</p>".to_string())).into_response(),
    }
}

async fn update_food_item_handler(
    AuthSession(_): AuthSession,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let mut name = String::new();
    let mut brand = String::new();
    let mut barcode: Option<String> = None;
    let mut calories = 0f64;
    let mut protein = 0f64;
    let mut carbs = 0f64;
    let mut fat = 0f64;
    let mut fiber = 0f64;
    let mut sugar = 0f64;
    let mut sodium = 0f64;
    let mut saturated_fat = 0f64;
    let mut package_size: Option<f64> = None;
    let mut custom_portions = String::new();
    let mut image_url = String::new();
    let mut image_bytes: Option<Vec<u8>> = None;

    while let Ok(Some(field)) = multipart.next_field().await {
        match field.name() {
            Some("name") => name = field.text().await.unwrap_or_default().trim().to_string(),
            Some("brand") => brand = field.text().await.unwrap_or_default().trim().to_string(),
            Some("barcode") => {
                let v = field.text().await.unwrap_or_default();
                let v = v.trim();
                if !v.is_empty() { barcode = Some(v.to_string()); }
            }
            Some("calories") => calories = field.text().await.unwrap_or_default().trim().parse().unwrap_or(0.0),
            Some("protein") => protein = field.text().await.unwrap_or_default().trim().parse().unwrap_or(0.0),
            Some("carbs") => carbs = field.text().await.unwrap_or_default().trim().parse().unwrap_or(0.0),
            Some("fat") => fat = field.text().await.unwrap_or_default().trim().parse().unwrap_or(0.0),
            Some("fiber") => fiber = field.text().await.unwrap_or_default().trim().parse().unwrap_or(0.0),
            Some("sugar") => sugar = field.text().await.unwrap_or_default().trim().parse().unwrap_or(0.0),
            Some("sodium") => sodium = field.text().await.unwrap_or_default().trim().parse().unwrap_or(0.0),
            Some("saturated_fat") => saturated_fat = field.text().await.unwrap_or_default().trim().parse().unwrap_or(0.0),
            Some("package_size") => {
                let v: f64 = field.text().await.unwrap_or_default().trim().parse().unwrap_or(0.0);
                if v > 0.0 { package_size = Some(v); }
            }
            Some("custom_portions") => custom_portions = field.text().await.unwrap_or_default().trim().to_string(),
            Some("image_url") => image_url = field.text().await.unwrap_or_default().trim().to_string(),
            Some("image") => {
                let bytes = field.bytes().await.unwrap_or_default();
                if !bytes.is_empty() {
                    image_bytes = Some(bytes.to_vec());
                }
            }
            _ => {}
        }
    }

    if name.is_empty() {
        return (StatusCode::BAD_REQUEST, Html("<p>Name is required</p>".to_string())).into_response();
    }

    // Upload new image to S3 if provided
    let mut uploaded_to_s3 = false;
    if let Some(bytes) = image_bytes {
        if let Some(ext) = crate::routes::admin::validate_magic_bytes(&bytes) {
            let ct = format!("image/{ext}");
            let key = format!("food/{}.{}", uuid::Uuid::new_v4(), ext);
            if let Ok(url) = state.storage.upload(&key, bytes, &ct).await {
                image_url = url;
                uploaded_to_s3 = true;
            }
        }
    }

    // Only allow OpenFoodFacts CDN URLs, our own S3 uploads, or empty
    if !image_url.is_empty()
        && !uploaded_to_s3
        && !image_url.starts_with("https://images.openfoodfacts.org/")
        && !image_url.starts_with("https://static.openfoodfacts.org/")
        && !image_url.starts_with("https://world.openfoodfacts.org/")
    {
        // Keep existing S3 image URL if it was already stored
        if let Some(existing) = crate::db::get_food_item(&state.pool, id).await {
            if image_url == existing.image_url {
                // URL unchanged, keep it
            } else {
                image_url = String::new();
            }
        } else {
            image_url = String::new();
        }
    }

    crate::db::update_food_item(
        &state.pool, id, &name, &brand, barcode.as_deref(),
        calories, protein, carbs, fat, fiber, sugar, sodium, saturated_fat, package_size, &custom_portions, &image_url,
    ).await;

    let all_items = crate::db::get_food_items(&state.pool).await;
    Html(library_list_html(&all_items, true)).into_response()
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/fitness", get(fitness_page))
        .route("/fitness/htmx/day", get(htmx_day))
        .route("/api/nutrition/food-items", post(add_food_item))
        .route("/api/nutrition/food-items/{id}", delete(delete_food_item_handler).put(update_food_item_handler))
        .route("/api/nutrition/food-items/{id}/edit", get(edit_food_form))
        .route("/api/nutrition/food-items/{id}/card", get(food_item_card))
        .route("/api/nutrition/entries", post(add_meal_entry))
        .route("/api/nutrition/entries/{id}", delete(delete_meal_entry_handler))
}
