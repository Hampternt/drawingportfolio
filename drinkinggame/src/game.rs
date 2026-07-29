//! Ring of Fire route handlers and the shared game-panel builder.
//! SQL stays in db.rs; HTML fragments stay in render.rs.

use axum::extract::{Form, Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Deserialize;

use crate::auth::PlayerSession;
use crate::cards;
use crate::db;
use crate::error::GameError;
use crate::hub::RoomMessage;
use crate::models::{Game, Player, Room};
use crate::render;
use crate::rules;
use crate::GameState;

/// Render the room's current game panel: active game state, or the idle
/// start panel when no game is running. `announcement` is transient — it
/// only appears in broadcast panels, never in page-load renders.
pub async fn current_panel(
    state: &GameState,
    room_id: i64,
    code: &str,
    announcement: Option<String>,
) -> String {
    match db::get_active_game(&state.pool, room_id).await {
        Some(game) => active_panel(state, &game, code, announcement).await,
        None => idle_panel(state, code).await,
    }
}

async fn idle_panel(state: &GameState, code: &str) -> String {
    let presets = db::list_presets(&state.pool).await;
    render::game_idle_panel(&state.base_path, code, &presets)
}

async fn active_panel(
    state: &GameState,
    game: &Game,
    code: &str,
    announcement: Option<String>,
) -> String {
    let deck = cards::parse_deck(&game.deck_order);
    let rules = rules::parse_rules(&game.rules_json);
    let draws = db::get_draws(&state.pool, game.id).await;
    let counts = db::draw_counts(&state.pool, game.id).await;

    let current = draws.last().map(|d| {
        let card = deck[d.card_index as usize];
        let rule = rules::rule_for_rank(&rules, card.rank);
        render::CurrentCard {
            card,
            title: rule.title.clone(),
            text: rule.text.clone(),
            drawer: d.player_name.clone(),
        }
    });
    let held = draws
        .iter()
        .filter(|d| d.spent_at.is_none())
        .filter_map(|d| {
            let card = deck[d.card_index as usize];
            let rule = rules::rule_for_rank(&rules, card.rank);
            rule.holdable.then(|| render::HeldCardView {
                draw_id: d.id,
                holder_id: d.player_id,
                holder_name: d.player_name.clone(),
                card,
                title: rule.title.clone(),
            })
        })
        .collect();

    let view = render::GameView {
        base_path: &state.base_path,
        code,
        current,
        remaining: 52 - draws.len() as i64,
        held,
        counts: &counts,
        announcement,
    };
    render::game_active_panel(&view)
}

async fn broadcast_panel(
    state: &GameState,
    room_id: i64,
    code: &str,
    announcement: Option<String>,
) {
    let html = current_panel(state, room_id, code, announcement).await;
    state.hub.publish(room_id, RoomMessage::Game(html));
}

/// End-of-game broadcast: summary on top, idle panel (Start button) below.
/// Page reloads render just the idle panel — the summary is transient.
async fn broadcast_game_over(state: &GameState, room_id: i64, code: &str, game_id: i64) {
    let counts = db::draw_counts(&state.pool, game_id).await;
    let html = format!(
        "{}{}",
        render::game_summary_panel(&counts),
        idle_panel(state, code).await
    );
    state.hub.publish(room_id, RoomMessage::Game(html));
}

/// Shared guard: open room + membership, mirroring log_event's checks.
async fn member_room(
    state: &GameState,
    code: &str,
    player: &Player,
) -> Result<Room, axum::response::Response> {
    let Some(room) = db::get_open_room(&state.pool, &code.to_uppercase()).await else {
        return Err(GameError::RoomNotFound.into_response());
    };
    if !db::is_room_member(&state.pool, room.id, player.id).await {
        return Err(StatusCode::FORBIDDEN.into_response());
    }
    Ok(room)
}

#[derive(Deserialize)]
pub struct StartForm {
    pub preset_id: i64,
}

pub async fn start_game_handler(
    State(state): State<GameState>,
    PlayerSession(player): PlayerSession,
    Path(code): Path<String>,
    Form(form): Form<StartForm>,
) -> axum::response::Response {
    let room = match member_room(&state, &code, &player).await {
        Ok(r) => r,
        Err(resp) => return resp,
    };
    let Some(preset) = db::get_preset(&state.pool, form.preset_id).await else {
        return GameError::PresetNotFound.into_response();
    };
    let deck = cards::deck_to_string(&cards::shuffled_deck());
    if let Err(e) = db::start_game(&state.pool, room.id, &preset.rules_json, &deck).await {
        return e.into_response();
    }
    db::touch_room(&state.pool, room.id).await;
    broadcast_panel(&state, room.id, &room.code, None).await;
    StatusCode::NO_CONTENT.into_response()
}

pub async fn draw_handler(
    State(state): State<GameState>,
    PlayerSession(player): PlayerSession,
    Path(code): Path<String>,
) -> axum::response::Response {
    let room = match member_room(&state, &code, &player).await {
        Ok(r) => r,
        Err(resp) => return resp,
    };
    let Some(game) = db::get_active_game(&state.pool, room.id).await else {
        return GameError::NoActiveGame.into_response();
    };
    let index = match db::insert_draw(&state.pool, game.id, player.id).await {
        Ok(i) => i,
        Err(e) => return e.into_response(),
    };
    db::touch_room(&state.pool, room.id).await;
    if index == 51 {
        // Last card: auto-end and broadcast the summary.
        db::end_game(&state.pool, game.id).await;
        broadcast_panel(&state, room.id, &room.code, None).await; // show the final card…
        broadcast_game_over(&state, room.id, &room.code, game.id).await; // …then the summary
    } else {
        broadcast_panel(&state, room.id, &room.code, None).await;
    }
    StatusCode::NO_CONTENT.into_response()
}

#[derive(Deserialize)]
pub struct SpendForm {
    pub draw_id: i64,
}

pub async fn spend_handler(
    State(state): State<GameState>,
    PlayerSession(player): PlayerSession,
    Path(code): Path<String>,
    Form(form): Form<SpendForm>,
) -> axum::response::Response {
    let room = match member_room(&state, &code, &player).await {
        Ok(r) => r,
        Err(resp) => return resp,
    };
    let Some(game) = db::get_active_game(&state.pool, room.id).await else {
        return GameError::NoActiveGame.into_response();
    };
    if !db::spend_draw(&state.pool, game.id, form.draw_id, player.id).await {
        return GameError::CardNotHeld.into_response();
    }
    db::touch_room(&state.pool, room.id).await;
    // Announce which rule was spent ("alice used Thumb Master!").
    let deck = cards::parse_deck(&game.deck_order);
    let rules = rules::parse_rules(&game.rules_json);
    let title = db::get_draws(&state.pool, game.id)
        .await
        .iter()
        .find(|d| d.id == form.draw_id)
        .map(|d| {
            rules::rule_for_rank(&rules, deck[d.card_index as usize].rank)
                .title
                .clone()
        })
        .unwrap_or_default();
    let announcement = format!("{} used {}!", player.name, title);
    broadcast_panel(&state, room.id, &room.code, Some(announcement)).await;
    StatusCode::NO_CONTENT.into_response()
}

pub async fn end_game_handler(
    State(state): State<GameState>,
    PlayerSession(player): PlayerSession,
    Path(code): Path<String>,
) -> axum::response::Response {
    let room = match member_room(&state, &code, &player).await {
        Ok(r) => r,
        Err(resp) => return resp,
    };
    let Some(game) = db::get_active_game(&state.pool, room.id).await else {
        return GameError::NoActiveGame.into_response();
    };
    db::end_game(&state.pool, game.id).await;
    db::touch_room(&state.pool, room.id).await;
    broadcast_game_over(&state, room.id, &room.code, game.id).await;
    StatusCode::NO_CONTENT.into_response()
}
