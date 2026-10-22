//! Drawing Tasks — a LeetCode-inspired practice board.
//!
//! Reference images are uploaded once; any number of tasks (prompts) can be
//! attached to each image in different categories — e.g. the same photo might
//! have a "draw only the hands" focus study, a "redraw in ink" style study,
//! and a "change the lighting" modification. Tasks are filterable by subject,
//! difficulty and task type, and sortable like a problem list.
//!
//! The page is a filter rail beside a grid of cards, the same arrangement as
//! `/artportfolio`. The rail lives inside `#task-board`, so every board swap
//! re-renders it with the board and its active chips and counts never freeze.

use crate::db::TaskFilters;
use crate::models::{DrawingTaskWithImage, TaskImage};
use crate::{
    middleware::{OptionalAdmin, RequireAdmin},
    AppState,
};
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

// ── View models ───────────────────────────────────────────────────────────────

const DIFFICULTIES: [&str; 3] = ["easy", "medium", "hard"];
const SORTS: [(&str, &str); 4] = [
    ("newest", "Newest"),
    ("oldest", "Oldest"),
    ("easiest", "Easiest"),
    ("hardest", "Hardest"),
];

fn difficulty_label(d: &str) -> &'static str {
    match d {
        "easy" => "Easy",
        "hard" => "Hard",
        _ => "Medium",
    }
}

/// The design system's badge tone for a difficulty: mint, amber, rose.
fn difficulty_tone(d: &str) -> &'static str {
    match d {
        "easy" => "success",
        "hard" => "danger",
        _ => "warning",
    }
}

/// The query string for a filter state. Empty fields and the default sort are
/// left out, so a clean board is plain `/tasks`.
fn filter_query(f: &TaskFilters) -> String {
    let mut s = url::form_urlencoded::Serializer::new(String::new());
    for (key, value) in [
        ("subject", &f.subject),
        ("difficulty", &f.difficulty),
        ("task_type", &f.task_type),
    ] {
        if !value.is_empty() {
            s.append_pair(key, value);
        }
    }
    if !f.sort.is_empty() && f.sort != "newest" {
        s.append_pair("sort", &f.sort);
    }
    s.finish()
}

/// A filter state as the two URLs a rail link carries: the page (for the
/// address bar and a no-JS click) and the board fragment (for the swap).
struct FilterLink {
    page_url: String,
    board_url: String,
}

impl FilterLink {
    fn new(f: &TaskFilters) -> Self {
        let q = filter_query(f);
        if q.is_empty() {
            FilterLink {
                page_url: "/tasks".into(),
                board_url: "/tasks/htmx/board".into(),
            }
        } else {
            FilterLink {
                page_url: format!("/tasks?{q}"),
                board_url: format!("/tasks/htmx/board?{q}"),
            }
        }
    }
}

/// One toggle in the rail. Its link is the full filter state a click
/// produces — this facet flipped, every other filter carried — so the
/// template never assembles a URL. `count` is how many tasks that state
/// would show; the sort chips carry none.
struct RailChip {
    label: String,
    count: Option<usize>,
    active: bool,
    link: FilterLink,
}

/// Which of the three facets a chip set toggles.
#[derive(Clone, Copy, PartialEq)]
enum Facet {
    Subject,
    Difficulty,
    TaskType,
}

impl Facet {
    fn get(self, f: &TaskFilters) -> &str {
        match self {
            Facet::Subject => &f.subject,
            Facet::Difficulty => &f.difficulty,
            Facet::TaskType => &f.task_type,
        }
    }

    fn of(self, t: &DrawingTaskWithImage) -> &str {
        match self {
            Facet::Subject => &t.subject,
            Facet::Difficulty => &t.difficulty,
            Facet::TaskType => &t.task_type,
        }
    }

    fn with(self, f: &TaskFilters, value: &str) -> TaskFilters {
        let mut next = TaskFilters {
            subject: f.subject.clone(),
            difficulty: f.difficulty.clone(),
            task_type: f.task_type.clone(),
            sort: f.sort.clone(),
        };
        let slot = match self {
            Facet::Subject => &mut next.subject,
            Facet::Difficulty => &mut next.difficulty,
            Facet::TaskType => &mut next.task_type,
        };
        *slot = value.to_string();
        next
    }
}

/// Does `t` pass every active filter except `skip`'s? Counting a chip this
/// way answers "what would I see if I picked this", which is the number a
/// filter rail should show.
fn matches_except(t: &DrawingTaskWithImage, f: &TaskFilters, skip: Facet) -> bool {
    [Facet::Subject, Facet::Difficulty, Facet::TaskType]
        .into_iter()
        .filter(|facet| *facet != skip)
        .all(|facet| facet.get(f).is_empty() || facet.of(t) == facet.get(f))
}

/// The chips for one facet, built from the unfiltered task list. `values` is
/// the order to show them in; an active value that no task carries any more
/// is still shown, at zero, so it can be cleared.
fn facet_chips(
    all: &[DrawingTaskWithImage],
    f: &TaskFilters,
    facet: Facet,
    values: Vec<String>,
    label: fn(&str) -> String,
) -> Vec<RailChip> {
    let mut values = values;
    let current = facet.get(f);
    if !current.is_empty() && !values.iter().any(|v| v == current) {
        values.push(current.to_string());
    }
    values
        .into_iter()
        .map(|v| {
            let active = v == current;
            let count = all
                .iter()
                .filter(|t| facet.of(t) == v && matches_except(t, f, facet))
                .count();
            // Clicking the active chip clears it.
            let next = facet.with(f, if active { "" } else { &v });
            RailChip {
                label: label(&v),
                count: Some(count),
                active,
                link: FilterLink::new(&next),
            }
        })
        .collect()
}

/// Distinct non-empty values of a facet across every task, sorted.
fn distinct(all: &[DrawingTaskWithImage], facet: Facet) -> Vec<String> {
    let mut v: Vec<String> = all
        .iter()
        .map(|t| facet.of(t).to_string())
        .filter(|s| !s.is_empty())
        .collect();
    v.sort();
    v.dedup();
    v
}

/// One task as its card renders it — the difficulty already resolved to a
/// label and a badge tone.
struct TaskCard<'a> {
    task: &'a DrawingTaskWithImage,
    difficulty_label: &'static str,
    difficulty_tone: &'static str,
}

/// Rail + grid. Everything the /tasks/htmx/board endpoint returns, swapped
/// into #task-board.
#[derive(Template)]
#[template(path = "tasks/partials/board.html")]
struct BoardTemplate<'a> {
    f: &'a TaskFilters,
    cards: Vec<TaskCard<'a>>,
    subjects: Vec<RailChip>,
    difficulties: Vec<RailChip>,
    types: Vec<RailChip>,
    sorts: Vec<RailChip>,
    total: usize,
    done: usize,
    done_percent: usize,
    shown_label: String,
    /// Some when any facet is active: the link that clears all three and
    /// keeps the sort.
    clear: Option<FilterLink>,
    is_admin: bool,
}

/// `all` is every task, unfiltered — the rail's counts and the progress line
/// come from it; `tasks` is what the current filters show.
fn board_html(
    all: &[DrawingTaskWithImage],
    tasks: &[DrawingTaskWithImage],
    f: &TaskFilters,
    is_admin: bool,
) -> String {
    let as_is = |s: &str| s.to_string();
    let subjects = facet_chips(all, f, Facet::Subject, distinct(all, Facet::Subject), as_is);
    let difficulties = facet_chips(
        all,
        f,
        Facet::Difficulty,
        DIFFICULTIES.iter().map(|d| d.to_string()).collect(),
        |d| difficulty_label(d).to_string(),
    );
    let types = facet_chips(all, f, Facet::TaskType, distinct(all, Facet::TaskType), as_is);

    let current_sort = if f.sort.is_empty() { "newest" } else { &f.sort };
    let sorts = SORTS
        .iter()
        .map(|(key, label)| RailChip {
            label: label.to_string(),
            count: None,
            active: *key == current_sort,
            link: FilterLink::new(&TaskFilters {
                subject: f.subject.clone(),
                difficulty: f.difficulty.clone(),
                task_type: f.task_type.clone(),
                sort: key.to_string(),
            }),
        })
        .collect();

    let total = all.len();
    let done = all.iter().filter(|t| t.completed).count();
    let done_percent = if total == 0 { 0 } else { done * 100 / total };
    let any_filter = !(f.subject.is_empty() && f.difficulty.is_empty() && f.task_type.is_empty());
    let shown_label = match (any_filter, tasks.len()) {
        (false, 1) => "1 task".to_string(),
        (false, n) => format!("{n} tasks"),
        (true, n) => format!("{n} of {total}"),
    };
    let clear = any_filter.then(|| {
        FilterLink::new(&TaskFilters {
            subject: String::new(),
            difficulty: String::new(),
            task_type: String::new(),
            sort: f.sort.clone(),
        })
    });

    let cards = tasks
        .iter()
        .map(|t| TaskCard {
            task: t,
            difficulty_label: difficulty_label(&t.difficulty),
            difficulty_tone: difficulty_tone(&t.difficulty),
        })
        .collect();

    BoardTemplate {
        f,
        cards,
        subjects,
        difficulties,
        types,
        sorts,
        total,
        done,
        done_percent,
        shown_label,
        clear,
        is_admin,
    }
    .render()
    .unwrap()
}

/// Admin panel: upload a reference image, attach a task to an existing image,
/// and manage (delete) uploaded images. Swapped into #task-admin.
#[derive(Template)]
#[template(path = "tasks/partials/admin.html")]
struct AdminTemplate<'a> {
    images: &'a [TaskImage],
    subjects: &'a [String],
    types: &'a [String],
}

fn admin_html(images: &[TaskImage], subjects: &[String], types: &[String]) -> String {
    AdminTemplate {
        images,
        subjects,
        types,
    }
    .render()
    .unwrap()
}

fn filters_from_params(params: &HashMap<String, String>) -> TaskFilters {
    TaskFilters {
        subject: params.get("subject").cloned().unwrap_or_default(),
        difficulty: params.get("difficulty").cloned().unwrap_or_default(),
        task_type: params.get("task_type").cloned().unwrap_or_default(),
        sort: params
            .get("sort")
            .cloned()
            .unwrap_or_else(|| "newest".to_string()),
    }
}

fn normalize_difficulty(d: &str) -> &'static str {
    match d {
        "easy" => "easy",
        "hard" => "hard",
        _ => "medium",
    }
}

async fn render_board(state: &AppState, f: &TaskFilters, is_admin: bool) -> String {
    let unfiltered = TaskFilters {
        subject: String::new(),
        difficulty: String::new(),
        task_type: String::new(),
        sort: String::new(),
    };
    let all = crate::db::get_tasks_filtered(&state.pool, &unfiltered).await;
    let tasks = crate::db::get_tasks_filtered(&state.pool, f).await;
    board_html(&all, &tasks, f, is_admin)
}

async fn render_admin(state: &AppState) -> String {
    let images = crate::db::get_task_images(&state.pool).await;
    let subjects = crate::db::get_task_subjects(&state.pool).await;
    let types = crate::db::get_task_types(&state.pool).await;
    admin_html(&images, &subjects, &types)
}

// ── Askama template ───────────────────────────────────────────────────────────

#[derive(Template)]
#[template(path = "tasks/feed.html")]
struct TasksTemplate {
    is_admin: bool,
    board_html: String,
    admin_html: String,
}

// ── Route handlers ────────────────────────────────────────────────────────────

/// Honours the same query as the board fragment, so a filtered board's
/// pushed URL reloads to the same board.
async fn tasks_page(
    OptionalAdmin(is_admin): OptionalAdmin,
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let f = filters_from_params(&params);
    let board = render_board(&state, &f, is_admin).await;
    let admin = if is_admin {
        render_admin(&state).await
    } else {
        String::new()
    };
    Html(
        TasksTemplate {
            is_admin,
            board_html: board,
            admin_html: admin,
        }
        .render()
        .unwrap(),
    )
}

async fn htmx_board(
    OptionalAdmin(is_admin): OptionalAdmin,
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let f = filters_from_params(&params);
    Html(render_board(&state, &f, is_admin).await)
}

async fn add_task_image(
    _: RequireAdmin,
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let mut title = String::new();
    let mut image_bytes: Option<Vec<u8>> = None;

    while let Ok(Some(field)) = multipart.next_field().await {
        match field.name() {
            Some("title") => title = field.text().await.unwrap_or_default().trim().to_string(),
            Some("image") => {
                let bytes = field.bytes().await.unwrap_or_default();
                if !bytes.is_empty() {
                    image_bytes = Some(bytes.to_vec());
                }
            }
            _ => {}
        }
    }

    if title.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Html("<p>Title is required</p>".to_string()),
        )
            .into_response();
    }
    let Some(bytes) = image_bytes else {
        return (
            StatusCode::BAD_REQUEST,
            Html("<p>Image is required</p>".to_string()),
        )
            .into_response();
    };
    let Some(ext) = crate::routes::admin::validate_magic_bytes(&bytes) else {
        return (
            StatusCode::BAD_REQUEST,
            Html("<p>Unsupported image format</p>".to_string()),
        )
            .into_response();
    };

    let ct = format!("image/{ext}");
    let key = format!("tasks/{}.{}", uuid::Uuid::new_v4(), ext);
    let url = match state.storage.upload(&key, bytes, &ct).await {
        Ok(u) => u,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html("<p>Upload failed</p>".to_string()),
            )
                .into_response()
        }
    };

    crate::db::insert_task_image(&state.pool, &title, &url).await;
    (
        [("HX-Trigger", "refresh-board")],
        Html(render_admin(&state).await),
    )
        .into_response()
}

async fn delete_task_image_handler(
    _: RequireAdmin,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    if let Some(url) = crate::db::delete_task_image(&state.pool, id).await {
        if !url.is_empty() {
            let _ = state.storage.delete_by_url(&url).await;
        }
    }
    (
        [("HX-Trigger", "refresh-board")],
        Html(render_admin(&state).await),
    )
}

async fn add_task(
    _: RequireAdmin,
    State(state): State<Arc<AppState>>,
    axum::Form(form): axum::Form<HashMap<String, String>>,
) -> impl IntoResponse {
    let image_id: i64 = form
        .get("image_id")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let title = form
        .get("title")
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    let prompt = form
        .get("prompt")
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    let subject = form
        .get("subject")
        .map(|s| s.trim().to_lowercase())
        .unwrap_or_default();
    let task_type = form
        .get("task_type")
        .map(|s| s.trim().to_lowercase())
        .unwrap_or_default();
    let difficulty = normalize_difficulty(
        form.get("difficulty")
            .map(String::as_str)
            .unwrap_or("medium"),
    );

    if image_id == 0 || title.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Html("<p>Image and title are required</p>".to_string()),
        )
            .into_response();
    }

    if !crate::db::insert_drawing_task(
        &state.pool,
        image_id,
        &title,
        &prompt,
        &subject,
        difficulty,
        &task_type,
    )
    .await
    {
        return (
            StatusCode::BAD_REQUEST,
            Html("<p>Reference image no longer exists</p>".to_string()),
        )
            .into_response();
    }
    (
        [("HX-Trigger", "refresh-board")],
        Html(render_admin(&state).await),
    )
        .into_response()
}

async fn delete_task_handler(
    _: RequireAdmin,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    crate::db::delete_drawing_task(&state.pool, id).await;
    let f = filters_from_params(&params);
    Html(render_board(&state, &f, true).await)
}

async fn toggle_task_handler(
    _: RequireAdmin,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    // htmx 2.x sends hx-include'd params in the body for POST requests
    axum::Form(params): axum::Form<HashMap<String, String>>,
) -> impl IntoResponse {
    crate::db::toggle_task_completed(&state.pool, id).await;
    let f = filters_from_params(&params);
    Html(render_board(&state, &f, true).await)
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/tasks", get(tasks_page))
        .route("/tasks/htmx/board", get(htmx_board))
        .route("/api/tasks", post(add_task))
        .route("/api/tasks/{id}", delete(delete_task_handler))
        .route("/api/tasks/{id}/toggle", post(toggle_task_handler))
        .route("/api/tasks/images", post(add_task_image))
        .route("/api/tasks/images/{id}", delete(delete_task_image_handler))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(id: i64, subject: &str, difficulty: &str, task_type: &str) -> DrawingTaskWithImage {
        DrawingTaskWithImage {
            id,
            image_id: 1,
            title: format!("Task {id}"),
            prompt: String::new(),
            subject: subject.into(),
            difficulty: difficulty.into(),
            task_type: task_type.into(),
            completed: false,
            created_at: String::new(),
            image_title: "Reference".into(),
            image_url: "/ref.webp".into(),
        }
    }

    fn filters(subject: &str, difficulty: &str, task_type: &str, sort: &str) -> TaskFilters {
        TaskFilters {
            subject: subject.into(),
            difficulty: difficulty.into(),
            task_type: task_type.into(),
            sort: sort.into(),
        }
    }

    fn sample() -> Vec<DrawingTaskWithImage> {
        vec![
            task(1, "figure", "easy", "gesture"),
            task(2, "figure", "hard", "modification"),
            task(3, "still life", "easy", "value study"),
        ]
    }

    #[test]
    fn test_filter_query_leaves_out_empty_fields_and_the_default_sort() {
        assert_eq!(filter_query(&filters("", "", "", "newest")), "");
        assert_eq!(filter_query(&filters("", "", "", "")), "");
        assert_eq!(
            filter_query(&filters("still life", "hard", "", "oldest")),
            "subject=still+life&difficulty=hard&sort=oldest"
        );
    }

    #[test]
    fn test_a_chip_counts_what_picking_it_would_show() {
        let all = sample();
        // Under difficulty=easy, "figure" would show one task, not two.
        let f = filters("", "easy", "", "newest");
        let chips = facet_chips(&all, &f, Facet::Subject, distinct(&all, Facet::Subject), |s| {
            s.to_string()
        });
        let figure = chips.iter().find(|c| c.label == "figure").unwrap();
        assert_eq!(figure.count, Some(1));
        assert!(!figure.active);
        // Its link adds the subject and keeps the difficulty.
        assert_eq!(figure.link.page_url, "/tasks?subject=figure&difficulty=easy");
        assert_eq!(
            figure.link.board_url,
            "/tasks/htmx/board?subject=figure&difficulty=easy"
        );
    }

    #[test]
    fn test_the_active_chip_clears_its_own_facet() {
        let all = sample();
        let f = filters("figure", "easy", "", "oldest");
        let chips = facet_chips(&all, &f, Facet::Subject, distinct(&all, Facet::Subject), |s| {
            s.to_string()
        });
        let figure = chips.iter().find(|c| c.label == "figure").unwrap();
        assert!(figure.active);
        assert_eq!(figure.link.page_url, "/tasks?difficulty=easy&sort=oldest");
    }

    #[test]
    fn test_an_active_value_no_task_carries_still_renders_so_it_can_be_cleared() {
        let all = sample();
        let f = filters("portrait", "", "", "newest");
        let chips = facet_chips(&all, &f, Facet::Subject, distinct(&all, Facet::Subject), |s| {
            s.to_string()
        });
        let gone = chips.iter().find(|c| c.label == "portrait").unwrap();
        assert!(gone.active);
        assert_eq!(gone.count, Some(0));
        assert_eq!(gone.link.page_url, "/tasks");
    }

    #[test]
    fn test_a_visitor_sees_no_admin_controls() {
        let all = sample();
        let f = filters("", "", "", "newest");
        let visitor = board_html(&all, &all, &f, false);
        assert!(visitor.contains("class=\"task-card"), "positive control");
        assert!(!visitor.contains("/toggle"));
        assert!(!visitor.contains("hx-delete"));
        let admin = board_html(&all, &all, &f, true);
        assert!(admin.contains("hx-post=\"/api/tasks/1/toggle\""));
        assert!(admin.contains("hx-delete=\"/api/tasks/1\""));
    }

    #[test]
    fn test_task_text_is_escaped() {
        let mut t = task(1, "<b>", "easy", "");
        t.title = "<script>alert(1)</script>".into();
        t.prompt = "a & b".into();
        let f = filters("", "", "", "newest");
        let html = board_html(std::slice::from_ref(&t), std::slice::from_ref(&t), &f, false);
        // Positive control: the text is there, only its markup is not.
        // (Askama escapes to numeric entities, so match the text alone.)
        assert!(html.contains("alert(1)"));
        assert!(!html.contains("<script>alert"));
        assert!(!html.contains("<b>"));
        assert!(!html.contains("a & b"));
    }

    #[test]
    fn test_the_board_carries_the_filter_state_its_buttons_include() {
        let all = sample();
        let f = filters("figure", "", "", "hardest");
        let html = board_html(&all, &all[..2], &f, true);
        assert!(html.contains("<form id=\"task-filters\" hidden>"));
        assert!(html.contains("name=\"subject\" value=\"figure\""));
        assert!(html.contains("name=\"sort\" value=\"hardest\""));
        assert!(html.contains("2 of 3"));
        assert!(html.contains("Clear filters"));
    }

    #[test]
    fn test_the_page_carries_the_site_dark_marker() {
        let html = TasksTemplate {
            is_admin: false,
            board_html: String::new(),
            admin_html: String::new(),
        }
        .render()
        .unwrap();
        assert!(html.contains("class=\"site-dark\""));
        let main = &html[html.find("<main").expect("a <main>")..];
        let first_div = &main[main.find("<div").unwrap()..];
        assert!(
            first_div.starts_with("<div class=\"site-page tasks-page\">"),
            "the .site-page marker must be the outermost element in the content block"
        );
        assert!(!html.contains("id=\"task-admin\""));
    }
}
