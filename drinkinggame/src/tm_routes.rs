//! 3 Man route handlers. `/tm/start` and `/tm/end` land here (Task 12);
//! the in-game action routes (`/tm/roll`, `/tm/pass`, `/tm/three-man`,
//! `/tm/mode`, `/tm/target`, `/tm/clear-slot`, `/tm/send`,
//! `/tm/gift-roll`, `/tm/seat`) are a later task. SQL stays in db.rs; HTML
//! fragments stay in render.rs.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;

use crate::auth::PlayerSession;
use crate::db;
use crate::error::GameError;
use crate::hub::RoomMessage;
use crate::models::{Game, Player, Room};
use crate::render;
use crate::three_man::ThreeManState;
use crate::GameState;

/// Everything an action handler needs to read and mutate a running 3 Man
/// game: the room (for id/code), the raw `games` row (for its id), and the
/// parsed state (mutated in place by the handler, then persisted).
pub(crate) struct TmCtx {
    pub room: Room,
    pub game: Game,
    pub st: ThreeManState,
}

/// member_room -> active game -> kind == "three_man" else WrongGameKind ->
/// parse state. Shared entry point for every `/tm/*` action handler.
pub(crate) async fn load_tm(
    state: &GameState,
    code: &str,
    player: &Player,
) -> Result<TmCtx, axum::response::Response> {
    let room = crate::game::member_room(state, code, player).await?;
    let Some(game) = db::get_active_game(&state.pool, room.id).await else {
        return Err(GameError::NoActiveGame.into_response());
    };
    if game.kind != "three_man" {
        return Err(GameError::WrongGameKind.into_response());
    }
    let st = ThreeManState::from_json(game.state_json.as_deref().unwrap_or_default());
    Ok(TmCtx { room, game, st })
}

/// Persists the mutated state back to the DB, then re-renders and publishes
/// every surface that reflects it: phone GAME tab, big screen, ROOM/TABLE
/// tab (topbar mode flip + seating), and the leaderboard's 3 MAN badge.
/// Shared by every action handler that mutates `ThreeManState` in place —
/// Task 13's roll/pass/hand-off/assign/gift-roll/seat routes, and
/// `tm_start_handler` below.
pub(crate) async fn persist_and_broadcast(state: &GameState, ctx: &TmCtx) {
    db::set_game_state(&state.pool, ctx.game.id, &ctx.st.to_json()).await;
    db::touch_room(&state.pool, ctx.room.id).await;
    crate::game::broadcast_game(state, ctx.room.id, &ctx.room.code, None).await;
    crate::game::broadcast_room(state, ctx.room.id, &ctx.room.code).await;
    crate::routes::broadcast_leaderboard(state, ctx.room.id).await;
}

/// member_room -> room_members (>= 2 else TooFewPlayers) ->
/// ThreeManState::new(member_ids_by_joined_at, player.id) -> start_game ->
/// touch, broadcast all. Locked across the whole body: a concurrent join
/// mutating the room's member list between the count check and the seed
/// must not race a start.
pub async fn tm_start_handler(
    State(state): State<GameState>,
    PlayerSession(player): PlayerSession,
    Path(code): Path<String>,
) -> axum::response::Response {
    let room = match crate::game::member_room(&state, &code, &player).await {
        Ok(r) => r,
        Err(resp) => return resp,
    };
    let lock = state.locks.for_room(room.id);
    let _guard = lock.lock().await;

    let members = db::room_members(&state.pool, room.id).await;
    if members.len() < 2 {
        return GameError::TooFewPlayers.into_response();
    }
    let member_ids: Vec<i64> = members.iter().map(|m| m.id).collect();
    let st = ThreeManState::new(member_ids, player.id);
    // deck_order/rules_json are Ring of Fire concepts — 3 Man leaves both
    // empty; GameAlreadyActive races are handled by the games table's
    // partial unique index (one active game per room).
    if let Err(e) = db::start_game(
        &state.pool,
        room.id,
        "three_man",
        "",
        "",
        Some(&st.to_json()),
    )
    .await
    {
        return e.into_response();
    }

    // Re-load under the lock rather than hand-assembling a Game/TmCtx: this
    // doubles as proof persist_and_broadcast works for a freshly-started
    // game, the same helper every future action handler reuses.
    let ctx = match load_tm(&state, &code, &player).await {
        Ok(c) => c,
        Err(resp) => return resp,
    };
    persist_and_broadcast(&state, &ctx).await;
    StatusCode::NO_CONTENT.into_response()
}

/// Assembles 3 Man post-game superlatives purely from the room leaderboard
/// — unlike Ring of Fire's `game_summary`, there's no draws/kings table to
/// join against (3 Man games never write to `game_draws`).
async fn tm_summary(state: &GameState, room_id: i64) -> render::TmOverView {
    let rows = db::leaderboard(&state.pool, room_id).await;
    let hardest = rows.first().map(|r| (r.name.clone(), r.drinks, r.shots));
    let most_shots = rows
        .iter()
        .max_by_key(|r| r.shots)
        .map(|r| (r.name.clone(), r.shots));
    let room_total: i64 = rows.iter().map(|r| r.drinks + r.shots).sum();
    render::TmOverView {
        hardest,
        most_shots,
        room_total,
    }
}

/// load_tm -> end_game -> build TmOverView from the leaderboard -> broadcast
/// the over panel + idle panel on `game`, the over panel on `screen`, then a
/// room refresh. Locked across the whole body: it can otherwise race a
/// gift_roll (Task 13) persisting state onto a game this request just
/// ended.
pub async fn tm_end_handler(
    State(state): State<GameState>,
    PlayerSession(player): PlayerSession,
    Path(code): Path<String>,
) -> axum::response::Response {
    let room = match crate::game::member_room(&state, &code, &player).await {
        Ok(r) => r,
        Err(resp) => return resp,
    };
    let lock = state.locks.for_room(room.id);
    let _guard = lock.lock().await;

    let ctx = match load_tm(&state, &code, &player).await {
        Ok(c) => c,
        Err(resp) => return resp,
    };
    db::end_game(&state.pool, ctx.game.id).await;
    db::touch_room(&state.pool, room.id).await;

    let summary = tm_summary(&state, room.id).await;
    let phone_html = format!(
        "{}{}",
        render::tm_over_panel(&summary),
        crate::game::idle_panel(&state, &room.code).await
    );
    let screen_html = render::tm_screen_over(&summary);
    state.hub.publish(room.id, RoomMessage::Game(phone_html));
    state.hub.publish(room.id, RoomMessage::Screen(screen_html));
    crate::game::broadcast_room(&state, room.id, &room.code).await;
    StatusCode::NO_CONTENT.into_response()
}
