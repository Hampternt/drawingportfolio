//! Room seat order and sitting out (table view, 2026-10-18).
//!
//! The seat ring belongs to the room, not to a game: one order every game
//! reads, carried over between games, changed from any signed-in phone.
//! The handlers here change it and keep a running 3 Man game — the only
//! engine that follows the ring mid-game — pointing at the same people.
//! Last Call and Backroom snapshot the ring when they start, so a change
//! made during one of them applies from the next game.
//!
//! Seat order is public, so everything here broadcasts like any other
//! visible change; nothing secret rides along.

use axum::extract::{Form, Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Deserialize;

use crate::auth::PlayerSession;
use crate::db;
use crate::error::GameError;
use crate::hub::RoomMessage;
use crate::models::Room;
use crate::three_man::{ThreeManState, TmError};
use crate::GameState;

/// Brings a running 3 Man game in line with the room's seat ring: seats
/// gone from the ring leave, new ones join, then the order follows the
/// ring. The roller and the 3 Man stay the same people throughout.
pub(crate) fn sync_three_man(st: &mut ThreeManState, seated: &[i64]) -> Result<(), TmError> {
    for id in st.order.clone() {
        if !seated.contains(&id) {
            st.remove_player(id)?;
        }
    }
    for &id in seated {
        st.add_player(id);
    }
    st.reorder(seated)
}

/// Re-reads the ring and applies it to a running 3 Man game, if there is
/// one. Caller holds the room lock. Returns whether a game state changed.
async fn sync_running_game(state: &GameState, room: &Room) -> Result<bool, TmError> {
    let Some(game) = db::get_active_game(&state.pool, room.id).await else {
        return Ok(false);
    };
    if game.kind != "three_man" {
        return Ok(false);
    }
    let mut st = ThreeManState::from_json(game.state_json.as_deref().unwrap_or_default());
    let before = st.clone();
    let seated: Vec<i64> = db::seated_members(&state.pool, room.id)
        .await
        .iter()
        .map(|m| m.id)
        .collect();
    sync_three_man(&mut st, &seated)?;
    if st == before {
        return Ok(false);
    }
    db::set_game_state(&state.pool, game.id, &st.to_json()).await;
    Ok(true)
}

/// Every surface that shows the ring: the ROOM tab, the standings (a
/// sit-out leaves them), and the game or the idle big screen.
///
/// With no game running only the screen frame goes out, never the phone's
/// game frame: that would replace a game-over summary still on everyone's
/// phone with the idle panel (`change_name` makes the same call).
pub(crate) async fn broadcast_seats(state: &GameState, room: &Room) {
    crate::game::broadcast_room(state, room.id, &room.code).await;
    crate::routes::broadcast_leaderboard(state, room.id).await;
    if db::get_active_game(&state.pool, room.id).await.is_some() {
        crate::game::broadcast_game(state, room.id, &room.code, None).await;
    } else {
        publish_idle_screen(state, room).await;
    }
}

/// With no game running, repaints the big screen's ring (a join, a seat
/// change). A no-op mid-game: each game's own broadcast owns the screen.
pub(crate) async fn publish_idle_screen(state: &GameState, room: &Room) {
    if db::get_active_game(&state.pool, room.id).await.is_some() {
        return;
    }
    let screen = crate::game::current_screen_panel(state, room.id, &room.code).await;
    state.hub.publish(room.id, RoomMessage::Screen(screen));
}

fn map_seat_error(e: TmError) -> GameError {
    match e {
        TmError::TooFewPlayers => GameError::TooFewPlayers,
        TmError::WrongPhase | TmError::BadTarget => GameError::NotNow,
    }
}

#[derive(Deserialize)]
pub struct MoveForm {
    pub player_id: i64,
    /// 1-based, as the dropdown shows it.
    pub position: usize,
}

/// `POST /room/{code}/seats/move` — the ROOM tab dropdown's "Seat N": the
/// player lands mid-gap at that slot. Any member may move anyone seated.
pub async fn move_handler(
    State(state): State<GameState>,
    PlayerSession(player): PlayerSession,
    Path(code): Path<String>,
    Form(form): Form<MoveForm>,
) -> axum::response::Response {
    let room = match crate::game::member_room(&state, &code, &player).await {
        Ok(r) => r,
        Err(resp) => return resp,
    };
    let lock = state.locks.for_room(room.id);
    let _guard = lock.lock().await;
    let position = form.position.saturating_sub(1);
    if !db::move_seat(&state.pool, room.id, form.player_id, position).await {
        return StatusCode::NO_CONTENT.into_response();
    }
    apply_and_broadcast(&state, &room).await;
    StatusCode::NO_CONTENT.into_response()
}

#[derive(Deserialize)]
pub struct AngleForm {
    /// Present for a token drop; absent for the start marker.
    pub player_id: Option<i64>,
    /// Degrees clockwise from the bottom of the table.
    pub angle: f64,
}

/// `POST /room/{code}/seats/angle` — a token dropped on the phone's table.
/// Sends just that player's new angle, never an order, so two phones
/// dragging at once both land. Any member may move anyone seated.
pub async fn angle_handler(
    State(state): State<GameState>,
    PlayerSession(player): PlayerSession,
    Path(code): Path<String>,
    Form(form): Form<AngleForm>,
) -> axum::response::Response {
    let Some(target) = form.player_id else {
        return StatusCode::UNPROCESSABLE_ENTITY.into_response();
    };
    place(&state, &player, &code, form.angle, Some(target)).await
}

/// `POST /room/{code}/seats/start` — the start marker dropped somewhere
/// else. Nobody moves; the count starts from the new spot.
pub async fn start_handler(
    State(state): State<GameState>,
    PlayerSession(player): PlayerSession,
    Path(code): Path<String>,
    Form(form): Form<AngleForm>,
) -> axum::response::Response {
    place(&state, &player, &code, form.angle, None).await
}

async fn place(
    state: &GameState,
    player: &crate::models::Player,
    code: &str,
    angle: f64,
    target: Option<i64>,
) -> axum::response::Response {
    if !angle.is_finite() {
        return StatusCode::UNPROCESSABLE_ENTITY.into_response();
    }
    let room = match crate::game::member_room(state, code, player).await {
        Ok(r) => r,
        Err(resp) => return resp,
    };
    let lock = state.locks.for_room(room.id);
    let _guard = lock.lock().await;
    let moved = match target {
        Some(id) => db::set_seat_angle(&state.pool, room.id, id, angle).await,
        None => db::set_start_angle(&state.pool, room.id, angle).await,
    };
    if moved {
        apply_and_broadcast(state, &room).await;
    }
    StatusCode::NO_CONTENT.into_response()
}

/// 3 Man's TABLE-tab ↑/↓: swaps `target`'s angle with its neighbour's in
/// the room's ring, wrapping between first and last.
pub(crate) async fn swap_with_neighbour(
    state: &GameState,
    room: &Room,
    target: i64,
    dir: i64,
) -> Result<(), GameError> {
    let seated: Vec<i64> = db::seated_members(&state.pool, room.id)
        .await
        .iter()
        .map(|m| m.id)
        .collect();
    let Some(i) = seated.iter().position(|&p| p == target) else {
        return Err(GameError::NotNow);
    };
    let len = seated.len() as i64;
    let j = (i as i64 + dir).rem_euclid(len) as usize;
    if i == j {
        return Ok(());
    }
    let other = seated[j];
    db::swap_seats(&state.pool, room.id, target, other).await;
    apply_and_broadcast(state, room).await;
    Ok(())
}

/// After a ring change that cannot fail the engine (a reorder, a join):
/// follow it in a running 3 Man game, then broadcast.
async fn apply_and_broadcast(state: &GameState, room: &Room) {
    if let Err(e) = sync_running_game(state, room).await {
        // Unreachable for a pure reorder (same players); logged rather
        // than surfaced, since the ring change itself already landed.
        tracing::warn!(
            "3 Man did not follow a seat change in room {}: {e:?}",
            room.id
        );
    }
    db::touch_room(&state.pool, room.id).await;
    broadcast_seats(state, room).await;
}

/// `POST /room/{code}/sit-out` — yourself only. A running 3 Man game drops
/// your seat first; if it can't right now (mid-double, or it would leave
/// one player) nothing changes and the phone shows why.
pub async fn sit_out_handler(
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
    if db::is_sitting_out(&state.pool, room.id, player.id).await {
        return StatusCode::NO_CONTENT.into_response();
    }
    // Check the engine before writing anything, so a refusal leaves the
    // ring and the game exactly as they were.
    if let Some(game) = db::get_active_game(&state.pool, room.id).await {
        if game.kind == "three_man" {
            let mut st = ThreeManState::from_json(game.state_json.as_deref().unwrap_or_default());
            if st.order.contains(&player.id) {
                if let Err(e) = st.remove_player(player.id) {
                    return map_seat_error(e).into_response();
                }
            }
        }
    }
    db::set_sitting_out(&state.pool, room.id, player.id, true).await;
    apply_and_broadcast(&state, &room).await;
    StatusCode::NO_CONTENT.into_response()
}

/// `POST /room/{code}/sit-in` — back to the seat you had. A running 3 Man
/// game seats you there too.
pub async fn sit_in_handler(
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
    if !db::set_sitting_out(&state.pool, room.id, player.id, false).await {
        return StatusCode::NO_CONTENT.into_response();
    }
    apply_and_broadcast(&state, &room).await;
    StatusCode::NO_CONTENT.into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_follows_the_ring_and_keeps_the_roller() {
        let mut st = ThreeManState::new(vec![1, 2, 3], 2); // order 2,3,1
        st.roll(2, 4).unwrap();
        st.pass().unwrap(); // 3 rolling
        sync_three_man(&mut st, &[3, 1, 2, 4]).unwrap();
        assert_eq!(st.order, vec![3, 1, 2, 4]);
        assert_eq!(st.roller(), 3);
        assert_eq!(st.three_man, 2);
    }

    #[test]
    fn test_sync_removes_a_sat_out_seat() {
        let mut st = ThreeManState::new(vec![1, 2, 3], 1);
        sync_three_man(&mut st, &[1, 3]).unwrap();
        assert_eq!(st.order, vec![1, 3]);
        assert_eq!(st.roller(), 1);
    }

    #[test]
    fn test_sync_refuses_mid_double() {
        let mut st = ThreeManState::new(vec![1, 2, 3], 1);
        st.roll(2, 2).unwrap();
        assert_eq!(sync_three_man(&mut st, &[1, 3]), Err(TmError::WrongPhase));
    }
}
