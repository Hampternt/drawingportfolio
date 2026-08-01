use crate::{middleware::AuthSession, AppState};
use askama::Template;
use axum::{
    extract::{Multipart, Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::{delete, get, post},
    Router,
};
use std::collections::HashMap;
use std::sync::Arc;

// ── HTML helpers ──────────────────────────────────────────────────────────────

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn fmt_nutrient(v: f64) -> String {
    if v == 0.0 {
        "0".to_string()
    } else {
        format!("{:.1}", v)
    }
}

pub fn food_item_card_html(item: &crate::models::FoodItem, is_admin: bool) -> String {
    let img_html = if item.image_url.is_empty() {
        String::new()
    } else {
        format!(
            "<img src=\"{}\" alt=\"{}\" class=\"food-thumb\" loading=\"lazy\">",
            html_escape(&item.image_url),
            html_escape(&item.name)
        )
    };
    let brand_html = if item.brand.is_empty() {
        String::new()
    } else {
        format!(
            "<span class=\"food-brand\">{}</span>",
            html_escape(&item.brand)
        )
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
        item.id,
        img_html,
        html_escape(&item.name),
        brand_html,
        fmt_nutrient(item.calories),
        fmt_nutrient(item.protein),
        fmt_nutrient(item.carbs),
        fmt_nutrient(item.fat),
        pkg_html,
        admin_btns
    )
}

const SLOTS: [(&str, &str); 5] = [
    ("breakfast", "Breakfast"),
    ("lunch", "Lunch"),
    ("dinner", "Dinner"),
    ("snack", "Snack"),
    ("other", "Other"),
];

pub fn meal_entry_row_html(
    entry: &crate::models::MealEntryWithFood,
    date: &str,
    is_admin: bool,
) -> String {
    let delete_btn = if is_admin {
        format!(
            "<button class=\"food-delete-btn\" hx-delete=\"/api/nutrition/entries/{}?date={}\" \
             hx-target=\"#day-section\" hx-swap=\"innerHTML\">×</button>",
            entry.entry_id,
            html_escape(date)
        )
    } else {
        String::new()
    };
    format!(
        r##"<li class="meal-entry" id="entry-{id}">
  <button type="button" class="entry-main" hx-get="/fitness/htmx/entries/{id}/edit?date={date}" hx-target="#entry-{id}" hx-swap="outerHTML">
    <span class="entry-name">{name}</span>
    <span class="entry-grams">{grams}g</span>
    <span class="entry-cal">{cal}</span>
  </button>
  {delete_btn}
</li>"##,
        id = entry.entry_id,
        date = html_escape(date),
        name = html_escape(&entry.food_name),
        grams = fmt_nutrient(entry.grams),
        cal = fmt_nutrient(entry.calories),
        delete_btn = delete_btn
    )
}

const RING_CIRC: f64 = 263.9; // 2π · 42 — matches the r="42" in calorie_ring_svg

fn ring_offset(consumed: f64, target: f64) -> f64 {
    let frac = if target > 0.0 {
        (consumed / target).clamp(0.0, 1.0)
    } else {
        0.0
    };
    RING_CIRC * (1.0 - frac)
}

fn rail_pct(value: f64, target: f64) -> f64 {
    if target <= 0.0 {
        return 0.0;
    }
    (value / target * 100.0).clamp(0.0, 100.0)
}

fn calorie_ring_svg(consumed: f64, target: f64) -> String {
    let offset = ring_offset(consumed, target);
    let remaining = (target - consumed).round();
    let (big, small) = if remaining >= 0.0 {
        (format!("{:.0}", remaining), "LEFT")
    } else {
        (format!("{:.0}", -remaining), "OVER")
    };
    // stroke hexes are the literal values of --noc-n800 / --noc-accent (SVG attrs can't read CSS vars from fragment strings)
    format!(
        r##"<svg class="cal-ring" width="98" height="98" viewBox="0 0 98 98" role="img" aria-label="{big} kcal {small_lc}">
  <circle cx="49" cy="49" r="42" fill="none" stroke="#3f424d" stroke-width="6"></circle>
  <circle cx="49" cy="49" r="42" fill="none" stroke="#9184d9" stroke-width="6" stroke-linecap="round" stroke-dasharray="{circ}" stroke-dashoffset="{offset:.1}" transform="rotate(-90 49 49)" style="filter:drop-shadow(0 0 6px rgba(145,132,217,.55))"></circle>
  <text x="49" y="46" text-anchor="middle" fill="#e9e9ed" font-size="21" font-weight="500">{big}</text>
  <text x="49" y="62" text-anchor="middle" fill="rgba(233,233,237,.5)" font-size="10" letter-spacing="0.08em">{small}</text>
</svg>"##,
        big = big,
        small = small,
        small_lc = small.to_lowercase(),
        circ = RING_CIRC,
        offset = offset
    )
}

fn macro_rail_html(label: &str, value: f64, target: f64, bar_hex: &str) -> String {
    format!(
        r##"<div class="macro-rail">
  <div class="rail-head"><span>{label}</span><span class="rail-nums">{v:.0} / {t:.0} g</span></div>
  <div class="rail-track"><div class="rail-fill" style="width:{pct:.0}%;background:{bar_hex}"></div></div>
</div>"##,
        label = label,
        v = value,
        t = target,
        pct = rail_pct(value, target),
        bar_hex = bar_hex
    )
}

/// The Sunday-first week containing `date`, as 7 (iso_date, kcal) pairs.
async fn week_for(pool: &crate::db::DbPool, date: &str) -> Vec<(String, f64)> {
    use chrono::{Datelike, Duration, NaiveDate};
    let d = NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .unwrap_or_else(|_| chrono::Utc::now().date_naive());
    let sunday = d - Duration::days(d.weekday().num_days_from_sunday() as i64);
    let days: Vec<String> = (0..7)
        .map(|i| (sunday + Duration::days(i)).format("%Y-%m-%d").to_string())
        .collect();
    let cals = crate::db::get_calories_by_date_range(pool, &days[0], &days[6]).await;
    days.into_iter()
        .map(|day| {
            let cal = cals
                .iter()
                .find(|(d2, _)| *d2 == day)
                .map(|(_, c)| *c)
                .unwrap_or(0.0);
            (day, cal)
        })
        .collect()
}

fn week_strip_html(week: &[(String, f64)], selected: &str, today: &str, target_cal: f64) -> String {
    const LETTERS: [&str; 7] = ["S", "M", "T", "W", "T", "F", "S"];
    let cols: String = week
        .iter()
        .enumerate()
        .map(|(i, (day, cal))| {
            let is_selected = day.as_str() == selected;
            let is_future = day.as_str() > today;
            let pct = if target_cal > 0.0 {
                ((cal / target_cal) * 100.0).clamp(0.0, 112.0)
            } else {
                0.0
            };
            let (cell_cls, fill) = if is_future {
                ("day-cell future", String::new())
            } else if is_selected {
                (
                    "day-cell selected",
                    format!(r#"<div class="day-fill accent" style="height:{pct:.0}%"></div>"#),
                )
            } else {
                (
                    "day-cell",
                    format!(r#"<div class="day-fill" style="height:{pct:.0}%"></div>"#),
                )
            };
            let letter_cls = if is_selected {
                "day-letter selected"
            } else if is_future {
                "day-letter future"
            } else {
                "day-letter"
            };
            format!(
                r##"<button type="button" class="day-col" data-date="{day}" onclick="loadDay('{day}')" aria-label="{day}">
  <span class="{letter_cls}">{letter}</span>
  <div class="{cell_cls}">{fill}</div>
</button>"##,
                day = day,
                letter = LETTERS[i],
                letter_cls = letter_cls,
                cell_cls = cell_cls,
                fill = fill
            )
        })
        .collect();
    format!(r#"<div class="week-strip" id="week-strip">{}</div>"#, cols)
}

pub fn day_section_html(
    entries: &[crate::models::MealEntryWithFood],
    date: &str,
    food_items: &[crate::models::FoodItem],
    targets: &crate::models::Targets,
    is_admin: bool,
) -> String {
    let total_cal: f64 = entries.iter().map(|e| e.calories).sum();
    let total_protein: f64 = entries.iter().map(|e| e.protein).sum();
    let total_carbs: f64 = entries.iter().map(|e| e.carbs).sum();
    let total_fat: f64 = entries.iter().map(|e| e.fat).sum();

    let slots_html: String = SLOTS
        .iter()
        .map(|(key, label)| {
            let slot_entries: Vec<_> = entries.iter().filter(|e| e.slot == *key).collect();
            if slot_entries.is_empty() && *key == "other" {
                return String::new(); // "other" group hidden when empty
            }
            let slot_cal: f64 = slot_entries.iter().map(|e| e.calories).sum();
            let head_right = if slot_entries.is_empty() {
                r#"<span class="slot-cal slot-empty">empty</span>"#.to_string()
            } else {
                format!(r#"<span class="slot-cal">{} cal</span>"#, fmt_nutrient(slot_cal))
            };
            let body = if slot_entries.is_empty() {
                format!(
                    r##"<button type="button" class="noc-btn noc-btn-secondary slot-add-btn" onclick="addToSlot('{key}')">+ Add to {label_lc}</button>"##,
                    key = key,
                    label_lc = label.to_lowercase()
                )
            } else {
                let rows: String = slot_entries
                    .iter()
                    .map(|e| meal_entry_row_html(e, date, is_admin))
                    .collect::<Vec<_>>()
                    .join("\n");
                format!("<ul class=\"meal-list\">\n{}\n</ul>", rows)
            };
            format!(
                r##"<div class="slot-group" id="slot-{key}">
  <div class="slot-head"><span class="noc-kicker">{label}</span>{head_right}</div>
  {body}
</div>"##,
                key = key,
                label = label,
                head_right = head_right,
                body = body
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let options_html: String = food_items
        .iter()
        .map(|fi| {
            let pkg_attr = if let Some(pkg) = fi.package_size {
                format!(" data-package-size=\"{}\"", pkg)
            } else {
                String::new()
            };
            let cp_attr = if fi.custom_portions.is_empty() {
                String::new()
            } else {
                format!(
                    " data-custom-portions=\"{}\"",
                    html_escape(&fi.custom_portions)
                )
            };
            format!(
                "<option value=\"{}\"{}{}>{} {}</option>",
                fi.id,
                pkg_attr,
                cp_attr,
                html_escape(&fi.name),
                if fi.brand.is_empty() {
                    String::new()
                } else {
                    format!("({})", html_escape(&fi.brand))
                }
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let pct_of_target = if targets.calories > 0.0 {
        (total_cal / targets.calories * 100.0).round()
    } else {
        0.0
    };
    let summary = format!(
        r##"<div class="day-summary noc-card">
  {ring}
  <div class="macro-rails">
    {p}{c}{f}
    <div class="cal-caption">{cal:.0} of {tcal:.0} cal · {pct:.0}%</div>
  </div>
</div>
<div class="targets-row">
  <button class="noc-btn noc-btn-ghost" hx-get="/fitness/htmx/targets?date={date}" hx-target="#targets-editor" hx-swap="innerHTML">Edit targets</button>
  <div id="targets-editor"></div>
</div>"##,
        ring = calorie_ring_svg(total_cal, targets.calories),
        p = macro_rail_html("Protein", total_protein, targets.protein, "#9184d9"),
        c = macro_rail_html("Carbs", total_carbs, targets.carbs, "#796cbf"),
        f = macro_rail_html("Fat", total_fat, targets.fat, "#5d5294"),
        cal = total_cal,
        tcal = targets.calories,
        pct = pct_of_target,
        date = html_escape(date)
    );

    format!(
        r##"{}
{}
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
  <input type="hidden" name="slot" value="other">
  <div class="slot-chips" data-role="slot-chips">
    <button type="button" class="noc-tag noc-tag-outline" data-slot="breakfast" onclick="setSlot(this)">Breakfast</button>
    <button type="button" class="noc-tag noc-tag-outline" data-slot="lunch" onclick="setSlot(this)">Lunch</button>
    <button type="button" class="noc-tag noc-tag-outline" data-slot="dinner" onclick="setSlot(this)">Dinner</button>
    <button type="button" class="noc-tag noc-tag-outline" data-slot="snack" onclick="setSlot(this)">Snack</button>
  </div>
  <button type="submit" class="btn-primary">Log</button>
</form>"##,
        summary,
        slots_html,
        html_escape(date),
        options_html
    )
}

pub fn library_list_html(items: &[crate::models::FoodItem], is_admin: bool) -> String {
    let cards: String = items
        .iter()
        .map(|i| food_item_card_html(i, is_admin))
        .collect::<Vec<_>>()
        .join("\n");
    format!("<ul class=\"food-library-list\">\n{}\n</ul>", cards)
}

fn edit_food_form_html(item: &crate::models::FoodItem) -> String {
    let barcode_val = item.barcode.as_deref().unwrap_or("");
    let pkg_val = item
        .package_size
        .map(|p| fmt_nutrient(p))
        .unwrap_or_default();
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
    date: String,
    week_strip_html: String,
    day_section_html: String,
    library_html: String,
}

// ── Route handlers ────────────────────────────────────────────────────────────

async fn fitness_page(
    AuthSession(_): AuthSession,
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    // ?date= selects the shown day; anything unparsable falls back to today
    let date = params
        .get("date")
        .filter(|d| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").is_ok())
        .cloned()
        .unwrap_or_else(|| today.clone());
    let entries = crate::db::get_meal_entries_for_date(&state.pool, &date).await;
    let food_items = crate::db::get_food_items(&state.pool).await;
    let targets = crate::db::get_targets(&state.pool).await;
    let week = week_for(&state.pool, &date).await;
    let strip = week_strip_html(&week, &date, &today, targets.calories);
    let day_html = day_section_html(&entries, &date, &food_items, &targets, true);
    let lib_html = library_list_html(&food_items, true);
    Html(
        FitnessTemplate {
            is_admin: true,
            today,
            date,
            week_strip_html: strip,
            day_section_html: day_html,
            library_html: lib_html,
        }
        .render()
        .unwrap(),
    )
}

async fn htmx_day(
    AuthSession(_): AuthSession,
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let date = params
        .get("date")
        .cloned()
        .unwrap_or_else(|| chrono::Utc::now().format("%Y-%m-%d").to_string());
    let entries = crate::db::get_meal_entries_for_date(&state.pool, &date).await;
    let food_items = crate::db::get_food_items(&state.pool).await;
    let targets = crate::db::get_targets(&state.pool).await;
    Html(day_section_html(
        &entries,
        &date,
        &food_items,
        &targets,
        true,
    ))
}

async fn add_food_item(
    AuthSession(_): AuthSession,
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
                if !v.is_empty() {
                    barcode = Some(v.to_string());
                }
            }
            Some("calories") => {
                calories = field
                    .text()
                    .await
                    .unwrap_or_default()
                    .trim()
                    .parse()
                    .unwrap_or(0.0)
            }
            Some("protein") => {
                protein = field
                    .text()
                    .await
                    .unwrap_or_default()
                    .trim()
                    .parse()
                    .unwrap_or(0.0)
            }
            Some("carbs") => {
                carbs = field
                    .text()
                    .await
                    .unwrap_or_default()
                    .trim()
                    .parse()
                    .unwrap_or(0.0)
            }
            Some("fat") => {
                fat = field
                    .text()
                    .await
                    .unwrap_or_default()
                    .trim()
                    .parse()
                    .unwrap_or(0.0)
            }
            Some("fiber") => {
                fiber = field
                    .text()
                    .await
                    .unwrap_or_default()
                    .trim()
                    .parse()
                    .unwrap_or(0.0)
            }
            Some("sugar") => {
                sugar = field
                    .text()
                    .await
                    .unwrap_or_default()
                    .trim()
                    .parse()
                    .unwrap_or(0.0)
            }
            Some("sodium") => {
                sodium = field
                    .text()
                    .await
                    .unwrap_or_default()
                    .trim()
                    .parse()
                    .unwrap_or(0.0)
            }
            Some("saturated_fat") => {
                saturated_fat = field
                    .text()
                    .await
                    .unwrap_or_default()
                    .trim()
                    .parse()
                    .unwrap_or(0.0)
            }
            Some("package_size") => {
                let v: f64 = field
                    .text()
                    .await
                    .unwrap_or_default()
                    .trim()
                    .parse()
                    .unwrap_or(0.0);
                if v > 0.0 {
                    package_size = Some(v);
                }
            }
            Some("custom_portions") => {
                custom_portions = field.text().await.unwrap_or_default().trim().to_string()
            }
            Some("image_url") => {
                image_url = field.text().await.unwrap_or_default().trim().to_string()
            }
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
        return (
            StatusCode::BAD_REQUEST,
            Html("<p>Name is required</p>".to_string()),
        )
            .into_response();
    }

    // Upload image to S3 if provided
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
        image_url = String::new();
    }

    let _item = crate::db::insert_food_item(
        &state.pool,
        &name,
        &brand,
        barcode.as_deref(),
        calories,
        protein,
        carbs,
        fat,
        fiber,
        sugar,
        sodium,
        saturated_fat,
        package_size,
        &custom_portions,
        &image_url,
    )
    .await;

    let all_items = crate::db::get_food_items(&state.pool).await;
    Html(library_list_html(&all_items, true)).into_response()
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
    AuthSession(_): AuthSession,
    State(state): State<Arc<AppState>>,
    axum::Form(form): axum::Form<HashMap<String, String>>,
) -> impl IntoResponse {
    let date = form
        .get("date")
        .cloned()
        .unwrap_or_else(|| chrono::Utc::now().format("%Y-%m-%d").to_string());
    let food_item_id: i64 = form
        .get("food_item_id")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let grams: f64 = form
        .get("grams")
        .and_then(|v| v.parse().ok())
        .unwrap_or(100.0);
    let slot = form
        .get("slot")
        .cloned()
        .unwrap_or_else(|| "other".to_string());
    let slot = if SLOTS.iter().any(|(k, _)| *k == slot) {
        slot
    } else {
        "other".to_string()
    };

    if food_item_id == 0 || grams <= 0.0 {
        let entries = crate::db::get_meal_entries_for_date(&state.pool, &date).await;
        let food_items = crate::db::get_food_items(&state.pool).await;
        let targets = crate::db::get_targets(&state.pool).await;
        return Html(day_section_html(
            &entries,
            &date,
            &food_items,
            &targets,
            true,
        ))
        .into_response();
    }

    let _ = crate::db::insert_meal_entry(&state.pool, food_item_id, &date, grams, &slot).await;
    let entries = crate::db::get_meal_entries_for_date(&state.pool, &date).await;
    let food_items = crate::db::get_food_items(&state.pool).await;
    let targets = crate::db::get_targets(&state.pool).await;
    Html(day_section_html(
        &entries,
        &date,
        &food_items,
        &targets,
        true,
    ))
    .into_response()
}

async fn delete_meal_entry_handler(
    AuthSession(_): AuthSession,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    crate::db::delete_meal_entry(&state.pool, id).await;
    let date = params
        .get("date")
        .cloned()
        .unwrap_or_else(|| chrono::Utc::now().format("%Y-%m-%d").to_string());
    let entries = crate::db::get_meal_entries_for_date(&state.pool, &date).await;
    let food_items = crate::db::get_food_items(&state.pool).await;
    let targets = crate::db::get_targets(&state.pool).await;
    Html(day_section_html(
        &entries,
        &date,
        &food_items,
        &targets,
        true,
    ))
}

async fn edit_food_form(
    AuthSession(_): AuthSession,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    match crate::db::get_food_item(&state.pool, id).await {
        Some(item) => Html(edit_food_form_html(&item)).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Html("<p>Food item not found</p>".to_string()),
        )
            .into_response(),
    }
}

async fn food_item_card(
    AuthSession(_): AuthSession,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    match crate::db::get_food_item(&state.pool, id).await {
        Some(item) => Html(food_item_card_html(&item, true)).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Html("<p>Food item not found</p>".to_string()),
        )
            .into_response(),
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
                if !v.is_empty() {
                    barcode = Some(v.to_string());
                }
            }
            Some("calories") => {
                calories = field
                    .text()
                    .await
                    .unwrap_or_default()
                    .trim()
                    .parse()
                    .unwrap_or(0.0)
            }
            Some("protein") => {
                protein = field
                    .text()
                    .await
                    .unwrap_or_default()
                    .trim()
                    .parse()
                    .unwrap_or(0.0)
            }
            Some("carbs") => {
                carbs = field
                    .text()
                    .await
                    .unwrap_or_default()
                    .trim()
                    .parse()
                    .unwrap_or(0.0)
            }
            Some("fat") => {
                fat = field
                    .text()
                    .await
                    .unwrap_or_default()
                    .trim()
                    .parse()
                    .unwrap_or(0.0)
            }
            Some("fiber") => {
                fiber = field
                    .text()
                    .await
                    .unwrap_or_default()
                    .trim()
                    .parse()
                    .unwrap_or(0.0)
            }
            Some("sugar") => {
                sugar = field
                    .text()
                    .await
                    .unwrap_or_default()
                    .trim()
                    .parse()
                    .unwrap_or(0.0)
            }
            Some("sodium") => {
                sodium = field
                    .text()
                    .await
                    .unwrap_or_default()
                    .trim()
                    .parse()
                    .unwrap_or(0.0)
            }
            Some("saturated_fat") => {
                saturated_fat = field
                    .text()
                    .await
                    .unwrap_or_default()
                    .trim()
                    .parse()
                    .unwrap_or(0.0)
            }
            Some("package_size") => {
                let v: f64 = field
                    .text()
                    .await
                    .unwrap_or_default()
                    .trim()
                    .parse()
                    .unwrap_or(0.0);
                if v > 0.0 {
                    package_size = Some(v);
                }
            }
            Some("custom_portions") => {
                custom_portions = field.text().await.unwrap_or_default().trim().to_string()
            }
            Some("image_url") => {
                image_url = field.text().await.unwrap_or_default().trim().to_string()
            }
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
        return (
            StatusCode::BAD_REQUEST,
            Html("<p>Name is required</p>".to_string()),
        )
            .into_response();
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
        &state.pool,
        id,
        &name,
        &brand,
        barcode.as_deref(),
        calories,
        protein,
        carbs,
        fat,
        fiber,
        sugar,
        sodium,
        saturated_fat,
        package_size,
        &custom_portions,
        &image_url,
    )
    .await;

    let all_items = crate::db::get_food_items(&state.pool).await;
    Html(library_list_html(&all_items, true)).into_response()
}

fn entry_edit_row_html(entry: &crate::models::MealEntry, food_name: &str, date: &str) -> String {
    let slot_opts: String = SLOTS
        .iter()
        .filter(|(k, _)| *k != "other" || entry.slot == "other")
        .map(|(k, l)| {
            format!(
                "<option value=\"{k}\"{sel}>{l}</option>",
                k = k,
                l = l,
                sel = if entry.slot == *k { " selected" } else { "" }
            )
        })
        .collect();
    format!(
        r##"<li class="meal-entry meal-entry-edit" id="entry-{id}">
<form hx-put="/api/nutrition/entries/{id}" hx-target="#day-section" hx-swap="innerHTML">
  <input type="hidden" name="date" value="{date}">
  <span class="entry-name">{name}</span>
  <input class="noc-input" type="number" name="grams" value="{grams}" min="1" max="5000" step="0.1" required>
  <select class="noc-input" name="slot">{slot_opts}</select>
  <button type="submit" class="noc-btn noc-btn-primary">Save</button>
  <button type="button" class="noc-btn noc-btn-ghost" hx-get="/fitness/htmx/day?date={date}" hx-target="#day-section" hx-swap="innerHTML">Cancel</button>
</form>
</li>"##,
        id = entry.id,
        date = html_escape(date),
        name = html_escape(food_name),
        grams = fmt_nutrient(entry.grams),
        slot_opts = slot_opts
    )
}

async fn entry_edit_form(
    AuthSession(_): AuthSession,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let date = params.get("date").cloned().unwrap_or_default();
    match crate::db::get_meal_entry(&state.pool, id).await {
        Some(entry) => {
            let name = crate::db::get_food_item(&state.pool, entry.food_item_id)
                .await
                .map(|f| f.name)
                .unwrap_or_default();
            Html(entry_edit_row_html(&entry, &name, &date)).into_response()
        }
        None => (
            StatusCode::NOT_FOUND,
            Html("<p>Entry not found</p>".to_string()),
        )
            .into_response(),
    }
}

async fn update_meal_entry_handler(
    AuthSession(_): AuthSession,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    axum::Form(form): axum::Form<HashMap<String, String>>,
) -> impl IntoResponse {
    let grams: f64 = form
        .get("grams")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.0);
    let slot = form
        .get("slot")
        .cloned()
        .unwrap_or_else(|| "other".to_string());
    let slot = if SLOTS.iter().any(|(k, _)| *k == slot) {
        slot
    } else {
        "other".to_string()
    };
    if grams > 0.0 {
        crate::db::update_meal_entry(&state.pool, id, grams, &slot).await;
    }
    let date = form
        .get("date")
        .cloned()
        .unwrap_or_else(|| chrono::Utc::now().format("%Y-%m-%d").to_string());
    let targets = crate::db::get_targets(&state.pool).await;
    let entries = crate::db::get_meal_entries_for_date(&state.pool, &date).await;
    let food_items = crate::db::get_food_items(&state.pool).await;
    Html(day_section_html(
        &entries,
        &date,
        &food_items,
        &targets,
        true,
    ))
}

/// The log card shown when a scan / search / recent tap resolves to a food item.
/// Portion buttons: package fractions and each custom portion; grams input as fallback.
fn match_card_html(item: &crate::models::FoodItem, kicker: &str) -> String {
    let mut portions: Vec<(String, f64)> = Vec::new();
    if let Some(pkg) = item.package_size {
        portions.push((format!("{} g", fmt_nutrient(pkg)), pkg));
        portions.push((format!("Half {} g", fmt_nutrient(pkg * 0.5)), pkg * 0.5));
    }
    for part in item.custom_portions.split(',') {
        if let Ok(g) = part.trim().parse::<f64>() {
            if g > 0.0 {
                portions.push((format!("{} g", fmt_nutrient(g)), g));
            }
        }
    }
    portions.truncate(3);
    let portion_btns: String = portions
        .iter()
        .enumerate()
        .map(|(i, (label, g))| {
            format!(
                r##"<button type="button" class="noc-btn {cls} portion-btn" data-grams="{g}" onclick="pickPortion(this)">{label}</button>"##,
                cls = if i == 0 { "noc-btn-primary" } else { "noc-btn-secondary" },
                g = g,
                label = label
            )
        })
        .collect();
    let default_grams = portions.first().map(|(_, g)| *g).unwrap_or(100.0);
    let brand = if item.brand.is_empty() {
        String::new()
    } else {
        format!("{} · ", html_escape(&item.brand))
    };
    format!(
        r##"<div class="match-card noc-card" id="match-card">
  <div class="match-head">
    <div class="match-title">{name}</div>
    <div class="match-sub">{brand}{cal} cal · P {p} · C {c} · F {f} / 100 g</div>
    <span class="noc-kicker">{kicker}</span>
  </div>
  <form hx-post="/api/nutrition/entries" hx-target="#day-section" hx-swap="innerHTML"
        hx-on::after-request="closeAddSheet()">
    <input type="hidden" name="date" value="">
    <input type="hidden" name="food_item_id" value="{id}">
    <input type="hidden" name="slot" value="other">
    <div class="noc-kicker">Portion</div>
    <div class="portion-row">{portion_btns}
      <input class="noc-input portion-grams" type="number" name="grams" value="{default_grams}" min="1" max="5000" step="0.1" required>
    </div>
    <div class="noc-kicker">Meal</div>
    <div class="slot-chips" data-role="slot-chips">
      <button type="button" class="noc-tag noc-tag-outline" data-slot="breakfast" onclick="setSlot(this)">Breakfast</button>
      <button type="button" class="noc-tag noc-tag-outline" data-slot="lunch" onclick="setSlot(this)">Lunch</button>
      <button type="button" class="noc-tag noc-tag-outline" data-slot="dinner" onclick="setSlot(this)">Dinner</button>
      <button type="button" class="noc-tag noc-tag-outline" data-slot="snack" onclick="setSlot(this)">Snack</button>
    </div>
    <button type="submit" class="noc-btn noc-btn-primary match-log-btn">Log it</button>
  </form>
</div>"##,
        name = html_escape(&item.name),
        brand = brand,
        cal = fmt_nutrient(item.calories),
        p = fmt_nutrient(item.protein),
        c = fmt_nutrient(item.carbs),
        f = fmt_nutrient(item.fat),
        kicker = html_escape(kicker),
        id = item.id,
        portion_btns = portion_btns,
        default_grams = fmt_nutrient(default_grams)
    )
}

async fn match_card(
    AuthSession(_): AuthSession,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    match crate::db::get_food_item(&state.pool, id).await {
        Some(item) => Html(match_card_html(&item, "From library")).into_response(),
        None => (StatusCode::NOT_FOUND, Html(String::new())).into_response(),
    }
}

async fn recent_chips(
    AuthSession(_): AuthSession,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let recents = crate::db::get_recent_foods(&state.pool, 8).await;
    let chips: String = recents
        .iter()
        .map(|r| {
            format!(
                r##"<button type="button" class="noc-btn noc-btn-secondary recent-chip"
             hx-get="/fitness/htmx/match-card/{id}" hx-target="#sheet-result" hx-swap="innerHTML">{name} {grams} g</button>"##,
                id = r.food_item_id,
                name = html_escape(&r.name),
                grams = fmt_nutrient(r.last_grams)
            )
        })
        .collect();
    Html(if chips.is_empty() {
        "<p class=\"sheet-hint\">Nothing logged yet.</p>".to_string()
    } else {
        chips
    })
}

async fn food_search(
    AuthSession(_): AuthSession,
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let q = params.get("q").cloned().unwrap_or_default();
    if q.trim().is_empty() {
        return Html(String::new());
    }
    let items = crate::db::search_food_items(&state.pool, q.trim()).await;
    let rows: String = items
        .iter()
        .map(|i| {
            format!(
                r##"<button type="button" class="search-row" data-item-id="{id}"
             hx-get="/fitness/htmx/match-card/{id}" hx-target="#sheet-result" hx-swap="innerHTML">
      <span class="search-name">{name}</span>
      <span class="search-macros">{cal} cal / 100 g</span>
    </button>"##,
                id = i.id,
                name = html_escape(&i.name),
                cal = fmt_nutrient(i.calories)
            )
        })
        .collect();
    Html(rows)
}

async fn barcode_match(
    AuthSession(_): AuthSession,
    State(state): State<Arc<AppState>>,
    Path(code): Path<String>,
) -> impl IntoResponse {
    match crate::db::get_food_item_by_barcode(&state.pool, &code).await {
        Some(item) => {
            let kicker = format!("Matched · {}", code);
            Html(match_card_html(&item, &kicker)).into_response()
        }
        None => (StatusCode::NOT_FOUND, Html(String::new())).into_response(),
    }
}

async fn copy_day_handler(
    AuthSession(_): AuthSession,
    State(state): State<Arc<AppState>>,
    axum::Form(form): axum::Form<HashMap<String, String>>,
) -> impl IntoResponse {
    let date = form
        .get("date")
        .cloned()
        .unwrap_or_else(|| chrono::Utc::now().format("%Y-%m-%d").to_string());
    let yesterday = chrono::NaiveDate::parse_from_str(&date, "%Y-%m-%d")
        .map(|d| {
            (d - chrono::Duration::days(1))
                .format("%Y-%m-%d")
                .to_string()
        })
        .unwrap_or_default();
    if !yesterday.is_empty() {
        crate::db::copy_day_entries(&state.pool, &yesterday, &date).await;
    }
    let targets = crate::db::get_targets(&state.pool).await;
    let entries = crate::db::get_meal_entries_for_date(&state.pool, &date).await;
    let food_items = crate::db::get_food_items(&state.pool).await;
    Html(day_section_html(
        &entries,
        &date,
        &food_items,
        &targets,
        true,
    ))
}

async fn targets_form(
    AuthSession(_): AuthSession,
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let date = params.get("date").cloned().unwrap_or_default();
    let t = crate::db::get_targets(&state.pool).await;
    Html(format!(
        r##"<form class="targets-form" hx-post="/api/nutrition/targets" hx-target="#day-section" hx-swap="innerHTML">
  <input type="hidden" name="date" value="{date}">
  <label>kcal<input class="noc-input" type="number" name="calories" min="0" step="1" value="{cal:.0}" required></label>
  <label>P g<input class="noc-input" type="number" name="protein" min="0" step="1" value="{p:.0}" required></label>
  <label>C g<input class="noc-input" type="number" name="carbs" min="0" step="1" value="{c:.0}" required></label>
  <label>F g<input class="noc-input" type="number" name="fat" min="0" step="1" value="{f:.0}" required></label>
  <button type="submit" class="noc-btn noc-btn-primary">Save</button>
</form>"##,
        date = html_escape(&date),
        cal = t.calories,
        p = t.protein,
        c = t.carbs,
        f = t.fat
    ))
}

async fn set_targets_handler(
    AuthSession(_): AuthSession,
    State(state): State<Arc<AppState>>,
    axum::Form(form): axum::Form<HashMap<String, String>>,
) -> impl IntoResponse {
    let g = |k: &str, d: f64| form.get(k).and_then(|v| v.parse().ok()).unwrap_or(d);
    crate::db::set_targets(
        &state.pool,
        g("calories", 2400.0),
        g("protein", 165.0),
        g("carbs", 260.0),
        g("fat", 72.0),
    )
    .await;
    let date = form
        .get("date")
        .cloned()
        .unwrap_or_else(|| chrono::Utc::now().format("%Y-%m-%d").to_string());
    let targets = crate::db::get_targets(&state.pool).await;
    let entries = crate::db::get_meal_entries_for_date(&state.pool, &date).await;
    let food_items = crate::db::get_food_items(&state.pool).await;
    Html(day_section_html(
        &entries,
        &date,
        &food_items,
        &targets,
        true,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ring_offset_bounds() {
        // full ring left at zero consumed, empty at/beyond target
        assert!((ring_offset(0.0, 2400.0) - 263.9).abs() < 0.1);
        assert!(ring_offset(2400.0, 2400.0).abs() < 0.1);
        assert!(ring_offset(3000.0, 2400.0).abs() < 0.1);
        // 77% consumed → 23% of the circumference remains as offset
        assert!((ring_offset(1848.0, 2400.0) - 60.7).abs() < 0.5);
    }

    #[test]
    fn test_rail_pct_clamps() {
        assert_eq!(rail_pct(0.0, 165.0), 0.0);
        assert_eq!(rail_pct(330.0, 165.0), 100.0);
        assert!((rail_pct(122.0, 165.0) - 73.9).abs() < 0.2);
        assert_eq!(rail_pct(50.0, 0.0), 0.0);
    }
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/fitness", get(fitness_page))
        .route("/fitness/htmx/day", get(htmx_day))
        .route("/fitness/htmx/targets", get(targets_form))
        .route("/api/nutrition/targets", post(set_targets_handler))
        .route("/api/nutrition/food-items", post(add_food_item))
        .route(
            "/api/nutrition/food-items/{id}",
            delete(delete_food_item_handler).put(update_food_item_handler),
        )
        .route("/api/nutrition/food-items/{id}/edit", get(edit_food_form))
        .route("/api/nutrition/food-items/{id}/card", get(food_item_card))
        .route("/api/nutrition/entries", post(add_meal_entry))
        .route(
            "/api/nutrition/entries/{id}",
            delete(delete_meal_entry_handler).put(update_meal_entry_handler),
        )
        .route("/fitness/htmx/entries/{id}/edit", get(entry_edit_form))
        .route("/fitness/copy-day", post(copy_day_handler))
        .route("/fitness/htmx/recent", get(recent_chips))
        .route("/fitness/htmx/food-search", get(food_search))
        .route("/fitness/htmx/match-card/{id}", get(match_card))
        .route("/fitness/htmx/barcode-match/{code}", get(barcode_match))
}
