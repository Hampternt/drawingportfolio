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

async fn tick(_state: &GameState, _room_id: i64) {
    // v1: nothing. Point-like mechanics land here without schema changes.
}
