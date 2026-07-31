//! Drinking game — party drink tracker, mounted under /drinks in the
//! portfolio server or served standalone via the bin target.

pub mod auth;
pub mod cards;
pub mod db;
pub mod error;
pub mod game;
pub mod hub;
pub mod mechanics;
pub mod models;
pub mod presets;
pub mod render;
pub mod rooms;
pub mod routes;
pub mod rules;
pub mod three_man;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::Mutex as TokioMutex;

/// Rooms idle longer than this are ended by the hourly sweep.
pub const MAX_IDLE_HOURS: i64 = 12;

/// Per-room async lock map for serializing access to room-specific resources.
/// Uses a std Mutex (never awaited) to guard the map, and Arc<tokio::sync::Mutex>
/// for each room's individual lock.
#[derive(Clone, Default)]
pub struct RoomLocks {
    inner: Arc<Mutex<HashMap<i64, Arc<TokioMutex<()>>>>>,
}

impl RoomLocks {
    /// Get or create an async lock for the given room.
    pub fn for_room(&self, room_id: i64) -> Arc<TokioMutex<()>> {
        let mut map = self.inner.lock().unwrap();
        map.entry(room_id)
            .or_insert_with(|| Arc::new(TokioMutex::new(())))
            .clone()
    }

    /// Remove a room's lock entry.
    pub fn remove(&self, room_id: i64) {
        self.inner.lock().unwrap().remove(&room_id);
    }
}

/// Everything the crate needs from its host. No portfolio types leak in here.
pub struct Config {
    /// e.g. "sqlite:./drinkinggame.db"
    pub database_url: String,
    /// URL prefix the router is mounted under: "" standalone, "/drinks" nested.
    /// Used only for URL generation in templates/redirects — routing itself
    /// is prefix-agnostic because .nest_service() strips the prefix.
    pub base_path: String,
}

#[derive(Clone)]
pub struct GameState {
    pub pool: db::DbPool,
    pub hub: hub::RoomHub,
    pub base_path: Arc<str>,
    pub locks: RoomLocks,
}

fn spawn_cleanup(state: GameState) {
    tokio::spawn(async move {
        // Hourly, mirroring the portfolio's cleanup cadence.
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600));
        loop {
            interval.tick().await;
            db::cleanup_expired_sessions(&state.pool).await;
            for room_id in db::end_inactive_rooms(&state.pool, MAX_IDLE_HOURS).await {
                state.hub.publish(room_id, hub::RoomMessage::Ended);
                state.hub.remove(room_id);
                state.locks.remove(room_id);
                tracing::info!("ended inactive room {room_id}");
            }
        }
    });
}

/// Build the game as a self-contained, stateless Router — the crate owns its
/// pool and hub, so this composes with any host via .nest_service().
pub async fn router(config: Config) -> axum::Router {
    let pool = db::connect(&config.database_url).await;
    db::run_migrations(&pool).await;
    router_with_pool(pool, &config.base_path)
}

/// Test seam: integration tests inject an in-memory pool here.
pub fn router_with_pool(pool: db::DbPool, base_path: &str) -> axum::Router {
    let state = GameState {
        pool,
        hub: hub::RoomHub::new(),
        base_path: Arc::from(base_path),
        locks: RoomLocks::default(),
    };
    spawn_cleanup(state.clone());
    mechanics::spawn_ticker(state.clone());
    routes::router().with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_room_locks_serialize_access() {
        let locks = RoomLocks::default();
        let ordering: Arc<std::sync::Mutex<Vec<&'static str>>> =
            Arc::new(std::sync::Mutex::new(Vec::new()));

        // Task 1
        let lock1 = locks.for_room(1);
        let ordering1 = ordering.clone();
        let task1 = tokio::spawn(async move {
            let _guard = lock1.lock().await;
            ordering1.lock().unwrap().push("start");
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            ordering1.lock().unwrap().push("end");
        });

        // Small delay to ensure task1 gets the lock first
        tokio::time::sleep(std::time::Duration::from_millis(1)).await;

        // Task 2
        let lock2 = locks.for_room(1);
        let ordering2 = ordering.clone();
        let task2 = tokio::spawn(async move {
            let _guard = lock2.lock().await;
            ordering2.lock().unwrap().push("start");
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            ordering2.lock().unwrap().push("end");
        });

        task1.await.unwrap();
        task2.await.unwrap();

        // Verify strict serialization: no interleaving
        let final_ordering = ordering.lock().unwrap();
        assert_eq!(*final_ordering, vec!["start", "end", "start", "end"]);
    }
}
