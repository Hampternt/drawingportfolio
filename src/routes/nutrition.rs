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

/// Valid ISO date from user input, else today (UTC). Handlers must not trust
/// the `date` form/query field — `date=""` or junk would otherwise create
/// entries no view can reach.
fn sanitize_date(input: Option<&String>) -> String {
    input
        .filter(|d| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").is_ok())
        .cloned()
        .unwrap_or_else(|| chrono::Utc::now().format("%Y-%m-%d").to_string())
}

/// The page head's micro-label: `MON 17 AUG · 2026-08-17`.
///
/// Both halves earn their place. The weekday and short date are what a person
/// reads; the ISO date is what every fragment URL on this page carries, so
/// printing it means a stale day-section is visible rather than merely wrong.
fn head_date_label(date: &str) -> String {
    match chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d") {
        Ok(d) => format!(
            "{} · {date}",
            d.format("%a %d %b").to_string().to_uppercase()
        ),
        // `sanitize_date` means this is unreachable from a request; falling back
        // to the raw string keeps the label honest if it ever is reached.
        Err(_) => date.to_string(),
    }
}

/// `date` shifted by `days`, ISO in and ISO out — the head's Yesterday link.
///
/// Server-side rather than the client-side `stepDay()` the arrows use, because
/// this one is an `href` that has to survive a page render with no JS.
fn step_date(date: &str, days: i64) -> String {
    chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .ok()
        .and_then(|d| d.checked_add_signed(chrono::Duration::days(days)))
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| date.to_string())
}

fn fmt_nutrient(v: f64) -> String {
    if v == 0.0 {
        "0".to_string()
    } else {
        format!("{:.1}", v)
    }
}

pub fn food_item_card_html(
    item: &crate::models::FoodItem,
    is_recent: bool,
    can_edit: bool,
) -> String {
    let img_html = if item.image_url.is_empty() {
        r#"<div class="food-thumb food-thumb-empty"></div>"#.to_string()
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
    let pkg_badge = if let Some(pkg) = item.package_size {
        format!(
            "<span class=\"noc-tag noc-tag-neutral food-pkg-badge\">{} g</span>",
            fmt_nutrient(pkg)
        )
    } else {
        String::new()
    };
    let admin_btns = if can_edit {
        format!(
            "<div class=\"food-admin-btns\">\
             <button class=\"fav-btn{fav_cls}\" hx-post=\"/api/nutrition/food-items/{id}/favourite\" \
             hx-target=\"#food-library\" hx-swap=\"innerHTML\" aria-label=\"Toggle favourite\">★</button>\
             <button class=\"food-edit-btn\" hx-get=\"/api/nutrition/food-items/{id}/edit\" \
             hx-target=\"#food-item-{id}\" hx-swap=\"outerHTML\">Edit</button>\
             <button class=\"food-delete-btn\" hx-delete=\"/api/nutrition/food-items/{id}\" \
             hx-target=\"#food-library\" hx-swap=\"innerHTML\" \
             hx-confirm=\"Delete this food item?\">×</button></div>",
            fav_cls = if item.is_favourite != 0 { " is-fav" } else { "" },
            id = item.id
        )
    } else {
        String::new()
    };
    format!(
        r##"<li class="food-item-card" id="food-item-{id}" data-fav="{fav}" data-recent="{rec}" data-protein="{prot}" data-cal="{cal_raw}">
  {img}
  <div class="food-info">
    <strong>{name} {brand}</strong>
    <span class="food-macros">{cal} cal · P {p}g · C {c}g · F {f}g</span>
  </div>
  {pkg}
  {admin}
</li>"##,
        id = item.id,
        fav = if item.is_favourite != 0 { 1 } else { 0 },
        rec = if is_recent { 1 } else { 0 },
        prot = item.protein,
        cal_raw = item.calories,
        img = img_html,
        name = html_escape(&item.name),
        brand = brand_html,
        cal = fmt_nutrient(item.calories),
        p = fmt_nutrient(item.protein),
        c = fmt_nutrient(item.carbs),
        f = fmt_nutrient(item.fat),
        pkg = pkg_badge,
        admin = admin_btns
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
    can_edit: bool,
) -> String {
    let delete_btn = if can_edit {
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

/// Consecutive logged days ending at `today` (or yesterday when today is
/// not yet logged). `logged_desc` is distinct dates, newest first.
fn compute_streak(logged_desc: &[String], today: &str) -> i64 {
    use chrono::{Duration, NaiveDate};
    let Ok(today) = NaiveDate::parse_from_str(today, "%Y-%m-%d") else {
        return 0;
    };
    let mut expect = today;
    let mut streak = 0i64;
    for (i, d) in logged_desc.iter().enumerate() {
        let Ok(d) = NaiveDate::parse_from_str(d, "%Y-%m-%d") else {
            break;
        };
        if i == 0 && d == today - Duration::days(1) {
            expect = d; // today not logged yet — start from yesterday
        }
        if d == expect {
            streak += 1;
            expect -= Duration::days(1);
        } else if d < expect {
            break;
        }
    }
    streak
}

/// The Sunday-first week containing `date`, as 7 (iso_date, kcal) pairs.
///
/// Takes the owner explicitly rather than reaching for a session: it is called
/// from several handlers, and a helper that quietly defaulted to "everyone"
/// would put the household's combined calories in one person's week strip.
async fn week_for(
    pool: &crate::db::DbPool,
    date: &str,
    user: crate::models::UserId,
) -> Vec<(String, f64)> {
    use chrono::{Datelike, Duration, NaiveDate};
    let d = NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .unwrap_or_else(|_| chrono::Utc::now().date_naive());
    let sunday = d - Duration::days(d.weekday().num_days_from_sunday() as i64);
    let days: Vec<String> = (0..7)
        .map(|i| (sunday + Duration::days(i)).format("%Y-%m-%d").to_string())
        .collect();
    let cals = crate::db::get_calories_by_date_range(pool, &days[0], &days[6], user).await;
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
    can_edit: bool,
) -> String {
    // The `+ 0.0` is not redundant. `f64`'s `Sum` identity is **negative** zero
    // — `-0.0 + x == x` holds for every x including `-0.0`, which `0.0` does
    // not satisfy — so an empty day sums to `-0.0`, `{:.0}` formats that as
    // "-0", and `rail_pct`'s `clamp(0.0, 100.0)` waves it through because
    // `-0.0 >= 0.0` is true. `-0.0 + 0.0 == 0.0` collapses it.
    //
    // The bug predates multi-user; what changed is who meets it. An empty day
    // used to be a date you had deliberately scrolled back to. It is now the
    // first screen every new member sees.
    let total_cal: f64 = entries.iter().map(|e| e.calories).sum::<f64>() + 0.0;
    let total_protein: f64 = entries.iter().map(|e| e.protein).sum::<f64>() + 0.0;
    let total_carbs: f64 = entries.iter().map(|e| e.carbs).sum::<f64>() + 0.0;
    let total_fat: f64 = entries.iter().map(|e| e.fat).sum::<f64>() + 0.0;

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
                    .map(|e| meal_entry_row_html(e, date, can_edit))
                    .collect::<Vec<_>>()
                    .join("\n");
                format!(
                    r##"<ul class="meal-list">
{rows}
</ul>
<details class="save-meal"><summary>Save as meal</summary>
<form hx-post="/api/nutrition/recipes" hx-target="#day-section" hx-swap="innerHTML">
  <input type="hidden" name="date" value="{date}"><input type="hidden" name="slot" value="{key}">
  <input class="noc-input" type="text" name="name" placeholder="Meal name" required>
  <button type="submit" class="noc-btn noc-btn-secondary">Save</button>
</form></details>"##,
                    rows = rows,
                    date = html_escape(date),
                    key = key
                )
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

pub fn library_list_html(
    items: &[crate::models::FoodItem],
    recent_ids: &std::collections::HashSet<i64>,
    can_edit: bool,
) -> String {
    use std::collections::BTreeMap;
    let mut groups: BTreeMap<String, Vec<&crate::models::FoodItem>> = BTreeMap::new();
    for item in items {
        // "zzz_" prefix sorts Uncategorised last; stripped before display
        let key = if item.category.is_empty() {
            "zzz_Uncategorised".to_string()
        } else {
            item.category.clone()
        };
        groups.entry(key).or_default().push(item);
    }
    groups
        .iter()
        .map(|(key, group)| {
            let label = key.strip_prefix("zzz_").unwrap_or(key);
            let cards: String = group
                .iter()
                .map(|i| food_item_card_html(i, recent_ids.contains(&i.id), can_edit))
                .collect::<Vec<_>>()
                .join("\n");
            format!(
                "<div class=\"lib-group\"><div class=\"noc-kicker lib-group-head\">{}</div>\n<ul class=\"food-library-list\">\n{}\n</ul></div>",
                html_escape(label),
                cards
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn history_bars_html(days: &[(String, f64)]) -> String {
    let max = days.iter().map(|(_, g)| *g).fold(0.0f64, f64::max).max(1.0);
    let bars: String = days
        .iter()
        .map(|(_, g)| {
            let pct = (g / max * 100.0).round();
            format!(
                "<div class=\"hist-bar\" style=\"height:{}%\"></div>",
                pct.max(if *g > 0.0 { 6.0 } else { 0.0 })
            )
        })
        .collect();
    format!(
        "<div class=\"item-history\"><div class=\"noc-kicker\">Last 14 days</div><div class=\"hist-strip\">{}</div></div>",
        bars
    )
}

fn edit_food_form_html(item: &crate::models::FoodItem, history_html: &str) -> String {
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
  <label class="package-size-label">Category<input type="text" name="category" value="{category}" placeholder="e.g. Dairy &amp; eggs"></label>
  <label class="package-size-label">Default portion (g)<input type="number" name="default_portion_g" step="0.1" min="0" value="{default_portion}" placeholder="usual amount"></label>
  <label class="fav-label"><input type="checkbox" name="is_favourite" value="1"{fav_checked}> Favourite</label>
  <label class="file-label">Image <input type="file" name="image" accept="image/jpeg,image/png,image/webp"></label>
  <input type="hidden" name="image_url" value="{image_url}">
  {history_html}
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
        category = html_escape(&item.category),
        default_portion = item.default_portion_g.map(fmt_nutrient).unwrap_or_default(),
        fav_checked = if item.is_favourite != 0 {
            " checked"
        } else {
            ""
        },
        history_html = history_html,
    )
}

// ── Askama template ───────────────────────────────────────────────────────────

#[derive(Template)]
#[template(path = "fitness/feed.html")]
struct FitnessTemplate {
    /// `base.html` reads this for its `IS_ADMIN` constant, which gates the
    /// command palette's admin-only entries. It is the *art-admin* question,
    /// not the "can I edit the food library" one — every signed-in user can do
    /// the latter, which is what `can_edit` means everywhere else in this file.
    is_admin: bool,
    user_name: String,
    date: String,
    /// `MON 17 AUG · 2026-08-17` — the page head's micro-label.
    date_label: String,
    /// Target of the head's Yesterday link.
    prev_date: String,
    week_strip_html: String,
    day_section_html: String,
    library_html: String,
}

// ── Route handlers ────────────────────────────────────────────────────────────

async fn fitness_page(
    session: AuthSession,
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let date = sanitize_date(params.get("date"));
    let entries = crate::db::get_meal_entries_for_date(&state.pool, &date, session.user()).await;
    let food_items = crate::db::get_food_items(&state.pool, session.user()).await;
    let targets = crate::db::get_targets(&state.pool, session.user()).await;
    let week = week_for(&state.pool, &date, session.user()).await;
    let strip = week_strip_html(&week, &date, &today, targets.calories);
    let day_html = day_section_html(&entries, &date, &food_items, &targets, true);
    let recent_ids: std::collections::HashSet<i64> =
        crate::db::get_recent_foods(&state.pool, 20, session.user())
            .await
            .into_iter()
            .map(|r| r.food_item_id)
            .collect();
    let lib_html = library_list_html(&food_items, &recent_ids, true);
    Html(
        FitnessTemplate {
            is_admin: session.is_effective_admin(),
            user_name: session.user_name,
            date_label: head_date_label(&date),
            prev_date: step_date(&date, -1),
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
    session: AuthSession,
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let date = sanitize_date(params.get("date"));
    let entries = crate::db::get_meal_entries_for_date(&state.pool, &date, session.user()).await;
    let food_items = crate::db::get_food_items(&state.pool, session.user()).await;
    let targets = crate::db::get_targets(&state.pool, session.user()).await;
    Html(day_section_html(
        &entries,
        &date,
        &food_items,
        &targets,
        true,
    ))
}

async fn add_food_item(
    session: AuthSession,
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
        session.user(),
    )
    .await;

    let all_items = crate::db::get_food_items(&state.pool, session.user()).await;
    {
        let recent_ids: std::collections::HashSet<i64> =
            crate::db::get_recent_foods(&state.pool, 20, session.user())
                .await
                .into_iter()
                .map(|r| r.food_item_id)
                .collect();
        Html(library_list_html(&all_items, &recent_ids, true)).into_response()
    }
}

async fn delete_food_item_handler(
    session: AuthSession,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    if let Some(img_url) = crate::db::delete_food_item(&state.pool, id).await {
        if !img_url.is_empty() {
            let _ = state.storage.delete_by_url(&img_url).await;
        }
    }
    let items = crate::db::get_food_items(&state.pool, session.user()).await;
    {
        let recent_ids: std::collections::HashSet<i64> =
            crate::db::get_recent_foods(&state.pool, 20, session.user())
                .await
                .into_iter()
                .map(|r| r.food_item_id)
                .collect();
        Html(library_list_html(&items, &recent_ids, true))
    }
}

async fn add_meal_entry(
    session: AuthSession,
    State(state): State<Arc<AppState>>,
    axum::Form(form): axum::Form<HashMap<String, String>>,
) -> impl IntoResponse {
    let date = sanitize_date(form.get("date"));
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
        let entries =
            crate::db::get_meal_entries_for_date(&state.pool, &date, session.user()).await;
        let food_items = crate::db::get_food_items(&state.pool, session.user()).await;
        let targets = crate::db::get_targets(&state.pool, session.user()).await;
        return Html(day_section_html(
            &entries,
            &date,
            &food_items,
            &targets,
            true,
        ))
        .into_response();
    }

    let _ = crate::db::insert_meal_entry(
        &state.pool,
        food_item_id,
        &date,
        grams,
        &slot,
        session.user(),
    )
    .await;
    let entries = crate::db::get_meal_entries_for_date(&state.pool, &date, session.user()).await;
    let food_items = crate::db::get_food_items(&state.pool, session.user()).await;
    let targets = crate::db::get_targets(&state.pool, session.user()).await;
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
    session: AuthSession,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    crate::db::delete_meal_entry(&state.pool, id, session.user()).await;
    let date = sanitize_date(params.get("date"));
    let entries = crate::db::get_meal_entries_for_date(&state.pool, &date, session.user()).await;
    let food_items = crate::db::get_food_items(&state.pool, session.user()).await;
    let targets = crate::db::get_targets(&state.pool, session.user()).await;
    Html(day_section_html(
        &entries,
        &date,
        &food_items,
        &targets,
        true,
    ))
}

async fn edit_food_form(
    session: AuthSession,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    match crate::db::get_food_item(&state.pool, id, session.user()).await {
        Some(item) => {
            use chrono::Duration;
            let today = chrono::Utc::now().date_naive();
            let start = (today - Duration::days(13)).format("%Y-%m-%d").to_string();
            let end = today.format("%Y-%m-%d").to_string();
            let logged =
                crate::db::get_item_log_history(&state.pool, id, &start, &end, session.user())
                    .await;
            let days: Vec<(String, f64)> = (0..14)
                .map(|i| {
                    let d = (today - Duration::days(13 - i))
                        .format("%Y-%m-%d")
                        .to_string();
                    let g = logged
                        .iter()
                        .find(|(ld, _)| *ld == d)
                        .map(|(_, g)| *g)
                        .unwrap_or(0.0);
                    (d, g)
                })
                .collect();
            Html(edit_food_form_html(&item, &history_bars_html(&days))).into_response()
        }
        None => (
            StatusCode::NOT_FOUND,
            Html("<p>Food item not found</p>".to_string()),
        )
            .into_response(),
    }
}

async fn food_item_card(
    session: AuthSession,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    match crate::db::get_food_item(&state.pool, id, session.user()).await {
        Some(item) => Html(food_item_card_html(&item, false, true)).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Html("<p>Food item not found</p>".to_string()),
        )
            .into_response(),
    }
}

async fn update_food_item_handler(
    session: AuthSession,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let mut category = String::new();
    let mut is_favourite = false;
    let mut default_portion_g: Option<f64> = None;
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
            Some("category") => {
                category = field.text().await.unwrap_or_default().trim().to_string()
            }
            Some("is_favourite") => {
                is_favourite = field.text().await.unwrap_or_default().trim() == "1"
            }
            Some("default_portion_g") => {
                let v: f64 = field
                    .text()
                    .await
                    .unwrap_or_default()
                    .trim()
                    .parse()
                    .unwrap_or(0.0);
                if v > 0.0 {
                    default_portion_g = Some(v);
                }
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
        if let Some(existing) = crate::db::get_food_item(&state.pool, id, session.user()).await {
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
        &category,
        is_favourite,
        default_portion_g,
        session.user(),
    )
    .await;

    let all_items = crate::db::get_food_items(&state.pool, session.user()).await;
    {
        let recent_ids: std::collections::HashSet<i64> =
            crate::db::get_recent_foods(&state.pool, 20, session.user())
                .await
                .into_iter()
                .map(|r| r.food_item_id)
                .collect();
        Html(library_list_html(&all_items, &recent_ids, true)).into_response()
    }
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
    session: AuthSession,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let date = sanitize_date(params.get("date"));
    match crate::db::get_meal_entry(&state.pool, id, session.user()).await {
        Some(entry) => {
            let name = crate::db::get_food_item(&state.pool, entry.food_item_id, session.user())
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
    session: AuthSession,
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
        crate::db::update_meal_entry(&state.pool, id, grams, &slot, session.user()).await;
    }
    let date = sanitize_date(form.get("date"));
    let targets = crate::db::get_targets(&state.pool, session.user()).await;
    let entries = crate::db::get_meal_entries_for_date(&state.pool, &date, session.user()).await;
    let food_items = crate::db::get_food_items(&state.pool, session.user()).await;
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
    if let Some(usual) = item.default_portion_g {
        if usual > 0.0 {
            portions.push((format!("Usual {} g", fmt_nutrient(usual)), usual));
        }
    }
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
        hx-on::after-request="if (event.detail.successful) closeAddSheet()">
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
    session: AuthSession,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    match crate::db::get_food_item(&state.pool, id, session.user()).await {
        Some(item) => Html(match_card_html(&item, "From library")).into_response(),
        None => (StatusCode::NOT_FOUND, Html(String::new())).into_response(),
    }
}

async fn recent_chips(
    session: AuthSession,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let recents = crate::db::get_recent_foods(&state.pool, 8, session.user()).await;
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
    session: AuthSession,
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let q = params.get("q").cloned().unwrap_or_default();
    if q.trim().is_empty() {
        return Html(String::new());
    }
    let items = crate::db::search_food_items(&state.pool, q.trim(), session.user()).await;
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
    session: AuthSession,
    State(state): State<Arc<AppState>>,
    Path(code): Path<String>,
) -> impl IntoResponse {
    match crate::db::get_food_item_by_barcode(&state.pool, &code, session.user()).await {
        Some(item) => {
            let kicker = format!("Matched · {}", code);
            Html(match_card_html(&item, &kicker)).into_response()
        }
        None => (StatusCode::NOT_FOUND, Html(String::new())).into_response(),
    }
}

async fn week_strip_fragment(
    session: AuthSession,
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let date = sanitize_date(params.get("date"));
    let targets = crate::db::get_targets(&state.pool, session.user()).await;
    let week = week_for(&state.pool, &date, session.user()).await;
    Html(week_strip_html(&week, &date, &today, targets.calories))
}

async fn quick_log_handler(
    session: AuthSession,
    State(state): State<Arc<AppState>>,
    axum::Form(form): axum::Form<HashMap<String, String>>,
) -> impl IntoResponse {
    let date = sanitize_date(form.get("date"));
    let slot = form
        .get("slot")
        .cloned()
        .unwrap_or_else(|| "other".to_string());
    let slot = if SLOTS.iter().any(|(k, _)| *k == slot) {
        slot
    } else {
        "other".to_string()
    };
    if let Some(id) = form.get("food_item_id").and_then(|v| v.parse::<i64>().ok()) {
        if let Some(item) = crate::db::get_food_item(&state.pool, id, session.user()).await {
            let grams = item.default_portion_g.unwrap_or(100.0);
            let _ =
                crate::db::insert_meal_entry(&state.pool, id, &date, grams, &slot, session.user())
                    .await;
        }
    }
    let targets = crate::db::get_targets(&state.pool, session.user()).await;
    let entries = crate::db::get_meal_entries_for_date(&state.pool, &date, session.user()).await;
    let food_items = crate::db::get_food_items(&state.pool, session.user()).await;
    Html(day_section_html(
        &entries,
        &date,
        &food_items,
        &targets,
        true,
    ))
}

#[derive(Template)]
#[template(path = "fitness/week.html")]
struct WeekTemplate {
    /// See `FitnessTemplate::is_admin` — the art-admin question, for the
    /// command palette.
    is_admin: bool,
    user_name: String,
    range_label: String,
    target_cal: String,
    avg_cal: String,
    bars_html: String,
    protein_avg: String,
    target_protein: String,
    protein_hits: i64,
    days_logged: i64,
    streak: i64,
    weight_card_html: String,
    most_logged_html: String,
}

fn weight_card_html(latest: Option<(String, f64)>, series: &[(String, f64)]) -> String {
    let (val, sub) = match &latest {
        Some((date, kg)) => (format!("{:.1}", kg), format!("kg · {}", html_escape(date))),
        None => ("—".to_string(), "no weight logged yet".to_string()),
    };
    let delta = if series.len() >= 2 {
        let d = series.last().unwrap().1 - series.first().unwrap().1;
        format!(
            r#"<span class="weight-delta">{}{:.1} kg / 30 d</span>"#,
            if d <= 0.0 { "−" } else { "+" },
            d.abs()
        )
    } else {
        String::new()
    };
    let line = if series.len() >= 2 {
        let min = series.iter().map(|(_, k)| *k).fold(f64::INFINITY, f64::min);
        let max = series
            .iter()
            .map(|(_, k)| *k)
            .fold(f64::NEG_INFINITY, f64::max);
        let span = (max - min).max(0.5);
        let pts: Vec<String> = series
            .iter()
            .enumerate()
            .map(|(i, (_, k))| {
                let x = i as f64 / (series.len() - 1) as f64 * 320.0;
                let y = 8.0 + (max - k) / span * 44.0;
                format!("{:.0},{:.1}", x, y)
            })
            .collect();
        format!(
            r##"<svg viewBox="0 0 320 60" width="100%" height="60" preserveAspectRatio="none"><polyline points="{}" fill="none" stroke="#9184d9" stroke-width="2" stroke-linecap="round" style="filter:drop-shadow(0 0 5px rgba(145,132,217,.5))"></polyline></svg>"##,
            pts.join(" ")
        )
    } else {
        String::new()
    };
    format!(
        r##"<div class="week-chart-head"><span class="noc-kicker">Weight</span>{delta}</div>
<div class="weight-now"><span class="stat-big">{val}</span><span class="stat-sub">{sub}</span></div>
{line}
<form class="weight-form" hx-post="/api/nutrition/weights" hx-target="#weight-card" hx-swap="innerHTML">
  <input class="noc-input" type="number" name="kg" step="0.1" min="20" max="400" placeholder="kg" required>
  <button type="submit" class="noc-btn noc-btn-secondary">Log today's weight</button>
</form>"##,
        delta = delta,
        val = val,
        sub = sub,
        line = line
    )
}

async fn week_page(session: AuthSession, State(state): State<Arc<AppState>>) -> impl IntoResponse {
    use chrono::{Duration, NaiveDate};
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let targets = crate::db::get_targets(&state.pool, session.user()).await;
    let week = week_for(&state.pool, &today, session.user()).await;
    let start = week[0].0.clone();
    let end = week[6].0.clone();

    let logged: Vec<&(String, f64)> = week
        .iter()
        .filter(|(d, c)| *c > 0.0 && d.as_str() <= today.as_str())
        .collect();
    let avg_cal = if logged.is_empty() {
        0.0
    } else {
        logged.iter().map(|(_, c)| c).sum::<f64>() / logged.len() as f64
    };

    let bars: String = week
        .iter()
        .map(|(d, c)| {
            let pct = if targets.calories > 0.0 {
                (c / targets.calories * 100.0).clamp(0.0, 112.0)
            } else {
                0.0
            };
            let cls = if *d == today {
                "wk-bar today"
            } else if d.as_str() > today.as_str() {
                "wk-bar future"
            } else {
                "wk-bar"
            };
            format!(
                r##"<a class="{cls}" href="/fitness?date={d}" style="--h:{pct:.0}%" aria-label="{d}"></a>"##,
                cls = cls,
                d = d,
                pct = pct
            )
        })
        .collect();
    // the target line sits at 100/112 of the clamped bar scale — fixed in CSS (bottom: 89.3%)
    let bars_html = format!(
        r##"<div class="wk-chart"><div class="wk-target-line"></div>{bars}</div>
<div class="wk-letters"><span>S</span><span>M</span><span>T</span><span>W</span><span>T</span><span>F</span><span>S</span></div>"##,
        bars = bars
    );

    let prot =
        crate::db::get_protein_by_date_range(&state.pool, &start, &end, session.user()).await;
    let protein_avg = if prot.is_empty() {
        0.0
    } else {
        prot.iter().map(|(_, p)| p).sum::<f64>() / prot.len() as f64
    };
    let protein_hits = prot.iter().filter(|(_, p)| *p >= targets.protein).count() as i64;
    let days_logged = logged.len() as i64;
    let streak = compute_streak(
        &crate::db::get_logged_dates_desc(&state.pool, 400, session.user()).await,
        &today,
    );

    let month_ago = NaiveDate::parse_from_str(&today, "%Y-%m-%d")
        .map(|d| (d - Duration::days(30)).format("%Y-%m-%d").to_string())
        .unwrap_or_default();
    let weights = crate::db::get_weights_since(&state.pool, &month_ago, session.user()).await;
    let latest = crate::db::get_latest_weight(&state.pool, session.user()).await;

    let most =
        crate::db::get_most_logged_between(&state.pool, &start, &end, 5, session.user()).await;
    let most_logged_html: String = most
        .iter()
        .map(|(name, n)| {
            format!(
                r#"<div class="most-row"><span class="most-name">{}</span><span class="most-n">{}×</span></div>"#,
                html_escape(name),
                n
            )
        })
        .collect();

    let range_label = {
        let s = NaiveDate::parse_from_str(&start, "%Y-%m-%d").unwrap();
        let e = NaiveDate::parse_from_str(&end, "%Y-%m-%d").unwrap();
        format!("{} – {}", s.format("%-d %b"), e.format("%-d %b"))
    };

    Html(
        WeekTemplate {
            is_admin: session.is_effective_admin(),
            user_name: session.user_name,
            range_label,
            target_cal: format!("{:.0}", targets.calories),
            avg_cal: format!("{:.0}", avg_cal),
            bars_html,
            protein_avg: format!("{:.0}", protein_avg),
            target_protein: format!("{:.0} g", targets.protein),
            protein_hits,
            days_logged,
            streak,
            weight_card_html: weight_card_html(latest, &weights),
            most_logged_html,
        }
        .render()
        .unwrap(),
    )
}

async fn log_weight_handler(
    session: AuthSession,
    State(state): State<Arc<AppState>>,
    axum::Form(form): axum::Form<HashMap<String, String>>,
) -> impl IntoResponse {
    use chrono::{Duration, NaiveDate};
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    if let Some(kg) = form.get("kg").and_then(|v| v.parse::<f64>().ok()) {
        if (20.0..=400.0).contains(&kg) {
            crate::db::upsert_weight(&state.pool, &today, kg, session.user()).await;
        }
    }
    let month_ago = NaiveDate::parse_from_str(&today, "%Y-%m-%d")
        .map(|d| (d - Duration::days(30)).format("%Y-%m-%d").to_string())
        .unwrap_or_default();
    let weights = crate::db::get_weights_since(&state.pool, &month_ago, session.user()).await;
    let latest = crate::db::get_latest_weight(&state.pool, session.user()).await;
    Html(weight_card_html(latest, &weights))
}

async fn create_recipe_handler(
    session: AuthSession,
    State(state): State<Arc<AppState>>,
    axum::Form(form): axum::Form<HashMap<String, String>>,
) -> impl IntoResponse {
    let name = form.get("name").map(|s| s.trim()).unwrap_or("");
    let date = sanitize_date(form.get("date"));
    let slot = form.get("slot").cloned().unwrap_or_default();
    if !name.is_empty() {
        crate::db::create_recipe_from_slot(&state.pool, name, &date, &slot, session.user()).await;
    }
    let targets = crate::db::get_targets(&state.pool, session.user()).await;
    let entries = crate::db::get_meal_entries_for_date(&state.pool, &date, session.user()).await;
    let food_items = crate::db::get_food_items(&state.pool, session.user()).await;
    Html(day_section_html(
        &entries,
        &date,
        &food_items,
        &targets,
        true,
    ))
}

async fn log_recipe_handler(
    session: AuthSession,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    axum::Form(form): axum::Form<HashMap<String, String>>,
) -> impl IntoResponse {
    let date = sanitize_date(form.get("date"));
    let slot = form
        .get("slot")
        .cloned()
        .unwrap_or_else(|| "other".to_string());
    let slot = if SLOTS.iter().any(|(k, _)| *k == slot) {
        slot
    } else {
        "other".to_string()
    };
    crate::db::log_recipe(&state.pool, id, &date, &slot, session.user()).await;
    let targets = crate::db::get_targets(&state.pool, session.user()).await;
    let entries = crate::db::get_meal_entries_for_date(&state.pool, &date, session.user()).await;
    let food_items = crate::db::get_food_items(&state.pool, session.user()).await;
    Html(day_section_html(
        &entries,
        &date,
        &food_items,
        &targets,
        true,
    ))
}

fn meals_pane_html(recipes: &[crate::models::RecipeWithTotals]) -> String {
    let rows: String = recipes
        .iter()
        .map(|r| {
            format!(
                r##"<div class="meal-row">
  <form hx-post="/api/nutrition/recipes/{id}/log" hx-target="#day-section" hx-swap="innerHTML" hx-on::after-request="if (event.detail.successful) closeAddSheet()">
    <input type="hidden" name="date" value=""><input type="hidden" name="slot" value="other">
    <button type="submit" class="noc-btn noc-btn-secondary meal-log-btn"><span>{name}</span><span class="meal-cal">{cal} cal</span></button>
  </form>
  <button class="food-delete-btn" hx-delete="/api/nutrition/recipes/{id}" hx-target="#sheet-meals .chips" hx-swap="innerHTML" hx-confirm="Delete this saved meal?">×</button>
</div>"##,
                id = r.id,
                name = html_escape(&r.name),
                cal = format!("{:.0}", r.total_cal)
            )
        })
        .collect();
    if rows.is_empty() {
        "<p class=\"sheet-hint\">No saved meals yet — save a day's slot from the Today view.</p>"
            .to_string()
    } else {
        rows
    }
}

async fn meals_pane(session: AuthSession, State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let recipes = crate::db::get_recipes_with_totals(&state.pool, session.user()).await;
    Html(meals_pane_html(&recipes))
}

async fn delete_recipe_handler(
    session: AuthSession,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    crate::db::delete_recipe(&state.pool, id, session.user()).await;
    let recipes = crate::db::get_recipes_with_totals(&state.pool, session.user()).await;
    Html(meals_pane_html(&recipes))
}

async fn toggle_favourite_handler(
    session: AuthSession,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    crate::db::toggle_food_favourite(&state.pool, id, session.user()).await;
    let items = crate::db::get_food_items(&state.pool, session.user()).await;
    let recent_ids: std::collections::HashSet<i64> =
        crate::db::get_recent_foods(&state.pool, 20, session.user())
            .await
            .into_iter()
            .map(|r| r.food_item_id)
            .collect();
    Html(library_list_html(&items, &recent_ids, true))
}

async fn favourite_chips(
    session: AuthSession,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let items = crate::db::get_food_items(&state.pool, session.user()).await;
    let chips: String = items
        .iter()
        .filter(|i| i.is_favourite != 0)
        .map(|i| {
            format!(
                r##"<button type="button" class="noc-btn noc-btn-secondary recent-chip"
             hx-get="/fitness/htmx/match-card/{id}" hx-target="#sheet-result" hx-swap="innerHTML">{name}</button>"##,
                id = i.id,
                name = html_escape(&i.name)
            )
        })
        .collect();
    Html(if chips.is_empty() {
        "<p class=\"sheet-hint\">No favourites yet — star items in the library.</p>".to_string()
    } else {
        chips
    })
}

async fn copy_day_handler(
    session: AuthSession,
    State(state): State<Arc<AppState>>,
    axum::Form(form): axum::Form<HashMap<String, String>>,
) -> impl IntoResponse {
    let date = sanitize_date(form.get("date"));
    let yesterday = chrono::NaiveDate::parse_from_str(&date, "%Y-%m-%d")
        .map(|d| {
            (d - chrono::Duration::days(1))
                .format("%Y-%m-%d")
                .to_string()
        })
        .unwrap_or_default();
    if !yesterday.is_empty() {
        crate::db::copy_day_entries(&state.pool, &yesterday, &date, session.user()).await;
    }
    let targets = crate::db::get_targets(&state.pool, session.user()).await;
    let entries = crate::db::get_meal_entries_for_date(&state.pool, &date, session.user()).await;
    let food_items = crate::db::get_food_items(&state.pool, session.user()).await;
    Html(day_section_html(
        &entries,
        &date,
        &food_items,
        &targets,
        true,
    ))
}

async fn targets_form(
    session: AuthSession,
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let date = sanitize_date(params.get("date"));
    let t = crate::db::get_targets(&state.pool, session.user()).await;
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
    session: AuthSession,
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
        session.user(),
    )
    .await;
    let date = sanitize_date(form.get("date"));
    let targets = crate::db::get_targets(&state.pool, session.user()).await;
    let entries = crate::db::get_meal_entries_for_date(&state.pool, &date, session.user()).await;
    let food_items = crate::db::get_food_items(&state.pool, session.user()).await;
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
    fn test_head_date_label_shape() {
        // 2026-08-17 is a Monday.
        assert_eq!(head_date_label("2026-08-17"), "MON 17 AUG · 2026-08-17");
        // Zero-padded day, and a month whose short name is not a prefix clash.
        assert_eq!(head_date_label("2026-09-05"), "SAT 05 SEP · 2026-09-05");
    }

    #[test]
    fn test_head_date_label_falls_back_to_the_raw_string() {
        // Unreachable through `sanitize_date`, but the label must never invent
        // a date it could not parse.
        assert_eq!(head_date_label("not-a-date"), "not-a-date");
    }

    #[test]
    fn test_step_date_crosses_month_and_year_boundaries() {
        assert_eq!(step_date("2026-08-17", -1), "2026-08-16");
        assert_eq!(step_date("2026-08-01", -1), "2026-07-31");
        assert_eq!(step_date("2026-01-01", -1), "2025-12-31");
        // Leap day: 2028 is a leap year, so stepping back from 1 March lands
        // on the 29th rather than skipping it.
        assert_eq!(step_date("2028-03-01", -1), "2028-02-29");
        assert_eq!(step_date("2026-08-17", 1), "2026-08-18");
    }

    /// An empty day must render `0`, not `-0`.
    ///
    /// `f64`'s `Sum` identity is negative zero, so every total on a day with no
    /// entries came out as `-0.0` and rendered as "-0 of 2400 cal · -0%" — with
    /// `rail_pct`'s `clamp(0.0, 100.0)` powerless to catch it, since `-0.0` is
    /// already inside the range. This is the first screen a new member sees.
    #[test]
    fn test_empty_day_renders_zero_not_negative_zero() {
        let targets = crate::models::Targets {
            calories: 2400.0,
            protein: 165.0,
            carbs: 260.0,
            fat: 72.0,
        };
        let html = day_section_html(&[], "2026-08-16", &[], &targets, true);

        // Checked against the rendered numbers rather than a bare `-0` search:
        // the ISO date in the markup contains "-0" all by itself.
        assert!(
            html.contains("0 of 2400 cal · 0%"),
            "calorie caption is wrong"
        );
        assert!(html.contains("<span>Protein</span><span class=\"rail-nums\">0 / 165 g"));
        assert!(html.contains("<span>Carbs</span><span class=\"rail-nums\">0 / 260 g"));
        assert!(html.contains("<span>Fat</span><span class=\"rail-nums\">0 / 72 g"));
        assert!(
            !html.contains("width:-0%"),
            "a rail was rendered with a negative-zero width"
        );
    }

    #[test]
    fn test_compute_streak() {
        let d = |s: &str| s.to_string();
        assert_eq!(compute_streak(&[], "2026-08-01"), 0);
        assert_eq!(
            compute_streak(
                &[d("2026-08-01"), d("2026-07-31"), d("2026-07-30")],
                "2026-08-01"
            ),
            3
        );
        // today not yet logged still counts yesterday's run
        assert_eq!(
            compute_streak(&[d("2026-07-31"), d("2026-07-30")], "2026-08-01"),
            2
        );
        // gap breaks it
        assert_eq!(
            compute_streak(&[d("2026-08-01"), d("2026-07-29")], "2026-08-01"),
            1
        );
    }

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
        .route("/fitness/htmx/favourites", get(favourite_chips))
        .route(
            "/api/nutrition/food-items/{id}/favourite",
            post(toggle_favourite_handler),
        )
        .route("/fitness/htmx/food-search", get(food_search))
        .route("/fitness/htmx/match-card/{id}", get(match_card))
        .route("/fitness/htmx/barcode-match/{code}", get(barcode_match))
        .route("/fitness/week", get(week_page))
        .route("/fitness/quick-log", post(quick_log_handler))
        .route("/fitness/htmx/week-strip", get(week_strip_fragment))
        .route("/api/nutrition/weights", post(log_weight_handler))
        .route("/api/nutrition/recipes", post(create_recipe_handler))
        .route("/api/nutrition/recipes/{id}/log", post(log_recipe_handler))
        .route("/api/nutrition/recipes/{id}", delete(delete_recipe_handler))
        .route("/fitness/htmx/meals", get(meals_pane))
}
