//! Rule-preset pages: list, create-as-copy, edit, delete. Auth-required but
//! not owner-scoped — it's a friends app; anyone logged in may edit.

use askama::Template;
use axum::extract::{Form, Path, State};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Redirect};
use serde::Deserialize;
use std::collections::HashMap;

use crate::auth::PlayerSession;
use crate::db;
use crate::render;
use crate::routes::error_page;
use crate::rules::RuleEntry;
use crate::GameState;

#[derive(Template)]
#[template(path = "presets.html")]
struct PresetsTemplate {
    base_path: String,
    preset_rows: String,
    source_options: String,
}

#[derive(Template)]
#[template(path = "preset_edit.html")]
struct PresetEditTemplate {
    base_path: String,
    id: i64,
    name: String,
    rank_rows: String,
}

pub async fn presets_page(
    State(state): State<GameState>,
    PlayerSession(_player): PlayerSession,
) -> impl IntoResponse {
    let presets = db::list_presets(&state.pool).await;
    let tpl = PresetsTemplate {
        base_path: state.base_path.to_string(),
        preset_rows: render::preset_rows(&state.base_path, &presets),
        source_options: render::preset_options(&presets),
    };
    Html(tpl.render().unwrap())
}

#[derive(Deserialize)]
pub struct CreateForm {
    pub name: String,
    pub source_id: i64,
}

pub async fn create_preset(
    State(state): State<GameState>,
    PlayerSession(_player): PlayerSession,
    Form(form): Form<CreateForm>,
) -> axum::response::Response {
    let name = form.name.trim();
    if name.is_empty() || name.chars().count() > 40 {
        return error_page(
            &state,
            StatusCode::UNPROCESSABLE_ENTITY,
            "preset name must be 1-40 characters",
        );
    }
    let Some(source) = db::get_preset(&state.pool, form.source_id).await else {
        return error_page(&state, StatusCode::NOT_FOUND, "no preset with that id");
    };
    match db::insert_preset(&state.pool, name, &source.rules_json).await {
        Ok(id) => Redirect::to(&format!("{}/presets/{id}", state.base_path)).into_response(),
        // UNIQUE name violation — the only insert error a user can cause.
        Err(_) => error_page(
            &state,
            StatusCode::CONFLICT,
            "a preset with that name already exists",
        ),
    }
}

pub async fn edit_preset_page(
    State(state): State<GameState>,
    PlayerSession(_player): PlayerSession,
    Path(id): Path<i64>,
) -> axum::response::Response {
    let Some(preset) = db::get_preset(&state.pool, id).await else {
        return error_page(&state, StatusCode::NOT_FOUND, "no preset with that id");
    };
    let rules = crate::rules::parse_rules(&preset.rules_json);
    let tpl = PresetEditTemplate {
        base_path: state.base_path.to_string(),
        id: preset.id,
        name: preset.name,
        rank_rows: render::preset_edit_rows(&rules),
    };
    Html(tpl.render().unwrap()).into_response()
}

pub async fn save_preset(
    State(state): State<GameState>,
    PlayerSession(_player): PlayerSession,
    Path(id): Path<i64>,
    Form(form): Form<HashMap<String, String>>,
) -> axum::response::Response {
    let name = form.get("name").map(|s| s.trim()).unwrap_or("");
    if name.is_empty() || name.chars().count() > 40 {
        return error_page(
            &state,
            StatusCode::UNPROCESSABLE_ENTITY,
            "preset name must be 1-40 characters",
        );
    }
    let mut rules = Vec::with_capacity(13);
    for rank in 1..=13u8 {
        let title = form
            .get(&format!("title_{rank}"))
            .map(|s| s.trim())
            .unwrap_or("");
        let text = form
            .get(&format!("text_{rank}"))
            .map(|s| s.trim())
            .unwrap_or("");
        if title.is_empty() || text.is_empty() {
            return error_page(
                &state,
                StatusCode::UNPROCESSABLE_ENTITY,
                "every rank needs a title and text",
            );
        }
        rules.push(RuleEntry {
            rank,
            title: title.to_string(),
            text: text.to_string(),
            // Unchecked checkboxes are simply absent from the form body.
            holdable: form.contains_key(&format!("holdable_{rank}")),
        });
    }
    let rules_json = serde_json::to_string(&rules).expect("rules serialize");
    match db::update_preset(&state.pool, id, name, &rules_json).await {
        Ok(true) => Redirect::to(&format!("{}/presets", state.base_path)).into_response(),
        Ok(false) => error_page(&state, StatusCode::NOT_FOUND, "no preset with that id"),
        Err(_) => error_page(
            &state,
            StatusCode::CONFLICT,
            "a preset with that name already exists",
        ),
    }
}

pub async fn delete_preset_handler(
    State(state): State<GameState>,
    PlayerSession(_player): PlayerSession,
    Path(id): Path<i64>,
) -> axum::response::Response {
    // Deleting is always allowed — running games hold snapshots, and the
    // migration guard recreates Standard on next deploy if it goes missing.
    db::delete_preset(&state.pool, id).await;
    Redirect::to(&format!("{}/presets", state.base_path)).into_response()
}
