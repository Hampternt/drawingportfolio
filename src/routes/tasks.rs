//! Drawing Tasks — a LeetCode-inspired practice board.
//!
//! Reference images are uploaded once; any number of tasks (prompts) can be
//! attached to each image in different categories — e.g. the same photo might
//! have a "draw only the hands" focus study, a "redraw in ink" style study,
//! and a "change the lighting" modification. Tasks are filterable by subject,
//! difficulty and task type, and sortable like a problem list.

use axum::{
    Router,
    routing::{get, post, delete},
    response::{Html, IntoResponse},
    extract::{State, Path, Query, Multipart},
    http::StatusCode,
};
use askama::Template;
use std::sync::Arc;
use std::collections::HashMap;
use crate::{AppState, middleware::{OptionalAuth, AuthSession}};
use crate::db::TaskFilters;
use crate::models::{DrawingTaskWithImage, TaskImage};

// ── HTML helpers ──────────────────────────────────────────────────────────────

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
     .replace('<', "&lt;")
     .replace('>', "&gt;")
     .replace('"', "&quot;")
}

fn difficulty_label(d: &str) -> &'static str {
    match d {
        "easy" => "Easy",
        "hard" => "Hard",
        _ => "Medium",
    }
}

fn selected_if(cond: bool) -> &'static str {
    if cond { " selected" } else { "" }
}

fn task_card_html(t: &DrawingTaskWithImage, is_admin: bool) -> String {
    let done_class = if t.completed { " task-done" } else { "" };
    let check = if t.completed { "<span class=\"task-check\" title=\"Completed\">✓</span>" } else { "" };
    let prompt_html = if t.prompt.is_empty() {
        String::new()
    } else {
        format!("<p class=\"task-prompt\">{}</p>", html_escape(&t.prompt))
    };
    let subject_tag = if t.subject.is_empty() {
        String::new()
    } else {
        format!("<span class=\"task-tag\">{}</span>", html_escape(&t.subject))
    };
    let type_tag = if t.task_type.is_empty() {
        String::new()
    } else {
        format!("<span class=\"task-tag\">{}</span>", html_escape(&t.task_type))
    };
    let admin_btns = if is_admin {
        let toggle_label = if t.completed { "Undo" } else { "Done" };
        format!(
            "<div class=\"task-admin-btns\">\
             <button class=\"task-toggle-btn\" hx-post=\"/api/tasks/{id}/toggle\" \
             hx-target=\"#task-board\" hx-swap=\"innerHTML\" hx-include=\"#task-filters\">{toggle_label}</button>\
             <button class=\"task-delete-btn\" hx-delete=\"/api/tasks/{id}\" \
             hx-target=\"#task-board\" hx-swap=\"innerHTML\" hx-include=\"#task-filters\" \
             hx-confirm=\"Delete this task?\">×</button></div>",
            id = t.id, toggle_label = toggle_label
        )
    } else {
        String::new()
    };
    format!(
        r#"<li class="task-card{done_class}" id="task-{id}">
  <a class="task-thumb-link" href="{image_url}" target="_blank" rel="noopener">
    <img class="task-thumb" src="{image_url}" alt="{image_title}" loading="lazy">
  </a>
  <div class="task-body">
    <div class="task-title-row">
      {check}<strong>{title}</strong>
      <span class="task-diff task-diff-{difficulty}">{diff_label}</span>
    </div>
    {prompt_html}
    <div class="task-tags">
      {subject_tag}{type_tag}<span class="task-img-label">{image_title}</span>
    </div>
  </div>
  {admin_btns}
</li>"#,
        done_class = done_class,
        id = t.id,
        image_url = html_escape(&t.image_url),
        image_title = html_escape(&t.image_title),
        check = check,
        title = html_escape(&t.title),
        difficulty = html_escape(&t.difficulty),
        diff_label = difficulty_label(&t.difficulty),
        prompt_html = prompt_html,
        subject_tag = subject_tag,
        type_tag = type_tag,
        admin_btns = admin_btns,
    )
}

/// Filter bar + task list. Everything the /tasks/htmx/board endpoint returns,
/// swapped into #task-board. The current filter values are re-marked as
/// `selected` so the form survives the swap.
pub fn board_html(
    tasks: &[DrawingTaskWithImage],
    subjects: &[String],
    types: &[String],
    f: &TaskFilters,
    is_admin: bool,
) -> String {
    let subject_options: String = subjects.iter()
        .map(|s| format!("<option value=\"{v}\"{sel}>{v}</option>",
            v = html_escape(s), sel = selected_if(*s == f.subject)))
        .collect();
    let type_options: String = types.iter()
        .map(|s| format!("<option value=\"{v}\"{sel}>{v}</option>",
            v = html_escape(s), sel = selected_if(*s == f.task_type)))
        .collect();

    let cards: String = tasks.iter()
        .map(|t| task_card_html(t, is_admin))
        .collect::<Vec<_>>()
        .join("\n");
    let list_html = if tasks.is_empty() {
        "<p class=\"empty-state\">No tasks match these filters.</p>".to_string()
    } else {
        format!("<ul class=\"task-list\">\n{}\n</ul>", cards)
    };

    let count = tasks.len();
    let count_label = if count == 1 { "task" } else { "tasks" };
    let done = tasks.iter().filter(|t| t.completed).count();

    format!(
        r##"<form id="task-filters" class="task-filters"
      hx-get="/tasks/htmx/board" hx-target="#task-board" hx-swap="innerHTML" hx-trigger="change">
  <select name="subject" aria-label="Filter by subject">
    <option value="">All subjects</option>
    {subject_options}
  </select>
  <select name="difficulty" aria-label="Filter by difficulty">
    <option value="">Any difficulty</option>
    <option value="easy"{sel_easy}>Easy</option>
    <option value="medium"{sel_medium}>Medium</option>
    <option value="hard"{sel_hard}>Hard</option>
  </select>
  <select name="task_type" aria-label="Filter by task type">
    <option value="">All types</option>
    {type_options}
  </select>
  <select name="sort" aria-label="Sort order">
    <option value="newest"{sort_newest}>Newest</option>
    <option value="oldest"{sort_oldest}>Oldest</option>
    <option value="easiest"{sort_easiest}>Easiest first</option>
    <option value="hardest"{sort_hardest}>Hardest first</option>
  </select>
  <span class="task-count">{done}/{count} {count_label} done</span>
</form>
{list_html}"##,
        subject_options = subject_options,
        sel_easy = selected_if(f.difficulty == "easy"),
        sel_medium = selected_if(f.difficulty == "medium"),
        sel_hard = selected_if(f.difficulty == "hard"),
        type_options = type_options,
        sort_newest = selected_if(f.sort == "newest" || f.sort.is_empty()),
        sort_oldest = selected_if(f.sort == "oldest"),
        sort_easiest = selected_if(f.sort == "easiest"),
        sort_hardest = selected_if(f.sort == "hardest"),
        done = done,
        count = count,
        count_label = count_label,
        list_html = list_html,
    )
}

/// Admin panel: upload a reference image, attach a task to an existing image,
/// and manage (delete) uploaded images. Swapped into #task-admin.
pub fn admin_html(images: &[TaskImage], subjects: &[String], types: &[String]) -> String {
    let image_options: String = images.iter()
        .map(|i| format!("<option value=\"{}\">{}</option>", i.id, html_escape(&i.title)))
        .collect();
    let subject_datalist: String = subjects.iter()
        .map(|s| format!("<option value=\"{}\"></option>", html_escape(s)))
        .collect();
    let type_datalist: String = types.iter()
        .map(|s| format!("<option value=\"{}\"></option>", html_escape(s)))
        .collect();
    let image_rows: String = images.iter()
        .map(|i| format!(
            r##"<li class="task-image-row">
  <img class="task-thumb" src="{url}" alt="{title}" loading="lazy">
  <span class="task-image-title">{title}</span>
  <button class="task-delete-btn" hx-delete="/api/tasks/images/{id}"
          hx-target="#task-admin" hx-swap="innerHTML"
          hx-confirm="Delete this image and ALL tasks attached to it?">×</button>
</li>"##,
            url = html_escape(&i.image_url), title = html_escape(&i.title), id = i.id
        ))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r##"<section class="task-admin">
  <h2>Manage tasks</h2>
  <div class="task-admin-forms">
    <form class="task-form" hx-post="/api/tasks/images" hx-target="#task-admin"
          hx-swap="innerHTML" hx-encoding="multipart/form-data">
      <h3>New reference image</h3>
      <input type="text" name="title" placeholder="Image title *" required>
      <label class="file-label">Image <input type="file" name="image" accept="image/jpeg,image/png,image/webp" required></label>
      <button type="submit" class="btn-primary">Upload image</button>
    </form>
    <form class="task-form" hx-post="/api/tasks" hx-target="#task-admin" hx-swap="innerHTML">
      <h3>New task</h3>
      <select name="image_id" required>
        <option value="">— pick reference image —</option>
        {image_options}
      </select>
      <input type="text" name="title" placeholder="Task title *" required>
      <textarea name="prompt" placeholder="Prompt — what to draw, which part to focus on, what style, what to change…"></textarea>
      <input type="text" name="subject" list="subject-options" placeholder="Subject (e.g. anatomy, portrait)">
      <datalist id="subject-options">{subject_datalist}</datalist>
      <input type="text" name="task_type" list="type-options" placeholder="Type (e.g. focus study, style study, modification)">
      <datalist id="type-options">{type_datalist}</datalist>
      <select name="difficulty" aria-label="Difficulty">
        <option value="easy">Easy</option>
        <option value="medium" selected>Medium</option>
        <option value="hard">Hard</option>
      </select>
      <button type="submit" class="btn-primary">Add task</button>
    </form>
  </div>
  <ul class="task-image-list">
{image_rows}
  </ul>
</section>"##,
        image_options = image_options,
        subject_datalist = subject_datalist,
        type_datalist = type_datalist,
        image_rows = image_rows,
    )
}

fn filters_from_params(params: &HashMap<String, String>) -> TaskFilters {
    TaskFilters {
        subject: params.get("subject").cloned().unwrap_or_default(),
        difficulty: params.get("difficulty").cloned().unwrap_or_default(),
        task_type: params.get("task_type").cloned().unwrap_or_default(),
        sort: params.get("sort").cloned().unwrap_or_else(|| "newest".to_string()),
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
    let tasks = crate::db::get_tasks_filtered(&state.pool, f).await;
    let subjects = crate::db::get_task_subjects(&state.pool).await;
    let types = crate::db::get_task_types(&state.pool).await;
    board_html(&tasks, &subjects, &types, f, is_admin)
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

async fn tasks_page(
    OptionalAuth(is_admin): OptionalAuth,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let f = filters_from_params(&HashMap::new());
    let board = render_board(&state, &f, is_admin).await;
    let admin = if is_admin { render_admin(&state).await } else { String::new() };
    Html(TasksTemplate {
        is_admin,
        board_html: board,
        admin_html: admin,
    }.render().unwrap())
}

async fn htmx_board(
    OptionalAuth(is_admin): OptionalAuth,
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let f = filters_from_params(&params);
    Html(render_board(&state, &f, is_admin).await)
}

async fn add_task_image(
    AuthSession(_): AuthSession,
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
        return (StatusCode::BAD_REQUEST, Html("<p>Title is required</p>".to_string())).into_response();
    }
    let Some(bytes) = image_bytes else {
        return (StatusCode::BAD_REQUEST, Html("<p>Image is required</p>".to_string())).into_response();
    };
    let Some(ext) = crate::routes::admin::validate_magic_bytes(&bytes) else {
        return (StatusCode::BAD_REQUEST, Html("<p>Unsupported image format</p>".to_string())).into_response();
    };

    let ct = format!("image/{ext}");
    let key = format!("tasks/{}.{}", uuid::Uuid::new_v4(), ext);
    let url = match state.storage.upload(&key, bytes, &ct).await {
        Ok(u) => u,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Html("<p>Upload failed</p>".to_string())).into_response(),
    };

    crate::db::insert_task_image(&state.pool, &title, &url).await;
    (
        [("HX-Trigger", "refresh-board")],
        Html(render_admin(&state).await),
    ).into_response()
}

async fn delete_task_image_handler(
    AuthSession(_): AuthSession,
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
    AuthSession(_): AuthSession,
    State(state): State<Arc<AppState>>,
    axum::Form(form): axum::Form<HashMap<String, String>>,
) -> impl IntoResponse {
    let image_id: i64 = form.get("image_id").and_then(|v| v.parse().ok()).unwrap_or(0);
    let title = form.get("title").map(|s| s.trim().to_string()).unwrap_or_default();
    let prompt = form.get("prompt").map(|s| s.trim().to_string()).unwrap_or_default();
    let subject = form.get("subject").map(|s| s.trim().to_lowercase()).unwrap_or_default();
    let task_type = form.get("task_type").map(|s| s.trim().to_lowercase()).unwrap_or_default();
    let difficulty = normalize_difficulty(form.get("difficulty").map(String::as_str).unwrap_or("medium"));

    if image_id == 0 || title.is_empty() {
        return (StatusCode::BAD_REQUEST, Html("<p>Image and title are required</p>".to_string())).into_response();
    }

    crate::db::insert_drawing_task(&state.pool, image_id, &title, &prompt, &subject, difficulty, &task_type).await;
    (
        [("HX-Trigger", "refresh-board")],
        Html(render_admin(&state).await),
    ).into_response()
}

async fn delete_task_handler(
    AuthSession(_): AuthSession,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    crate::db::delete_drawing_task(&state.pool, id).await;
    let f = filters_from_params(&params);
    Html(render_board(&state, &f, true).await)
}

async fn toggle_task_handler(
    AuthSession(_): AuthSession,
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
