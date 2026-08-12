//! Idle-mechanics extension point. V1 ships this empty on purpose — future
//! mechanics fold over the append-only event log and publish through the
//! same broadcast hub the leaderboard uses.

use crate::GameState;

/// Called after every successfully logged event.
pub fn on_event(_room_id: i64, _player_id: i64, _kind: &str) {}

/// One global 1 Hz ticker iterating rooms with live subscribers — bounded
/// work, no per-room task spawning. Spawned by router_with_pool (Task 11).
pub fn spawn_ticker(state: GameState) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
        loop {
            interval.tick().await;
            for room_id in state.hub.active_rooms() {
                tick(&state, room_id).await;
            }
        }
    });
}

/// Plan E's extension point: the Last Call beat clock. Ring of Fire and 3
/// Man carry no per-second mechanic (rounds turn on explicit actions), so
/// `lc_tick_room` — which returns immediately for any room not running an
/// active Last Call game — is the entire body.
async fn tick(state: &GameState, room_id: i64) {
    crate::lc_routes::lc_tick_room(state, room_id).await;
}
