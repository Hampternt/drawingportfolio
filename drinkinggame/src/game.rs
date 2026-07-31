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
    let house_rules = db::house_rules(&state.pool, game.id).await;

    let spends = draws.iter().filter(|d| d.spent_at.is_some()).count();
    let anim_key = format!("{}-{}", draws.len(), spends);

    let current = draws.last().map(|d| {
        let card = deck[d.card_index as usize];
        let rule = rules::rule_for_rank(&rules, card.rank);
        // Jack (rank 11) with no house_rules row for this draw yet: the
        // drawer still owes the room a rule.
        let pending_rule = card.rank == 11 && !house_rules.iter().any(|hr| hr.draw_id == d.id);
        render::CurrentCard {
            card,
            title: rule.title.clone(),
            text: rule.text.clone(),
            drawer: d.player_name.clone(),
            drawer_id: d.player_id,
            draw_id: d.id,
            pending_rule,
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
        anim_key,
    };
    render::game_active_panel(&view)
}

/// Room-wide ROOM/TABLE tab panel: members, house rules, King's Cup fill,
/// and a `mode` derived from whatever game (if any) is active. Task 5 wires
/// this into broadcasts; Task 7 wires it into the room shell.
pub async fn current_room_panel(state: &GameState, room_id: i64, code: &str) -> String {
    let members = db::room_members(&state.pool, room_id).await;
    let (house_rules, kings, mode) = match db::get_active_game(&state.pool, room_id).await {
        Some(game) => (
            db::house_rules(&state.pool, game.id).await,
            db::king_count(&state.pool, game.id).await,
            game.kind,
        ),
        None => (Vec::new(), 0, "idle".to_string()),
    };
    let view = render::RoomView {
        base_path: &state.base_path,
        code,
        members: &members,
        house_rules: &house_rules,
        kings,
        mode: &mode,
    };
    render::room_panel(&view)
}

/// Assembles post-game superlatives from three independent sources: draw
/// counts (hardest hit / most draws), the room leaderboard (most shots /
/// room total), and the King's Cup drawer. `room_total` is drinks+shots
/// logged room-wide this session — not cards drawn, which `counts` already
/// covers per-player.
async fn game_summary(state: &GameState, room_id: i64, game_id: i64) -> render::GameSummary {
    let counts = db::draw_counts(&state.pool, game_id).await;
    let hardest = counts.first().map(|c| (c.name.clone(), c.draws));
    let leaderboard = db::leaderboard(&state.pool, room_id).await;
    let most_shots = leaderboard
        .iter()
        .filter(|r| r.shots > 0)
        .max_by_key(|r| r.shots)
        .map(|r| (r.name.clone(), r.shots));
    let room_total: i64 = leaderboard.iter().map(|r| r.drinks + r.shots).sum();
    let kings_cup = db::last_king_drawer(&state.pool, game_id).await;
    let house_rules = db::house_rules(&state.pool, game_id).await;
    render::GameSummary {
        hardest,
        most_shots,
        room_total,
        kings_cup,
        counts,
        house_rules,
    }
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
    let summary = game_summary(state, room_id, game_id).await;
    let html = format!(
        "{}{}",
        render::game_over_panel(&summary),
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
    if let Err(e) = db::start_game(
        &state.pool,
        room.id,
        "ring_of_fire",
        &preset.rules_json,
        &deck,
        None,
    )
    .await
    {
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
    let ranks: Vec<u8> = cards::parse_deck(&game.deck_order)
        .iter()
        .map(|c| c.rank)
        .collect();
    let index = match db::insert_draw(&state.pool, game.id, player.id, &ranks).await {
        Ok(i) => i,
        Err(e) => return e.into_response(),
    };
    db::touch_room(&state.pool, room.id).await;
    if index == 51 {
        // Last card: render + publish the active panel showing the 52nd card
        // BEFORE ending the game. broadcast_panel/current_panel goes through
        // db::get_active_game, which filters ended_at IS NULL — calling
        // end_game first would make this "final card" frame render the idle
        // panel instead of the card that was just drawn.
        let html = active_panel(&state, &game, &room.code, None).await;
        state.hub.publish(room.id, RoomMessage::Game(html));
        db::end_game(&state.pool, game.id).await;
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
