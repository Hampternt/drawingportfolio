//! Drinking game — party drink tracker, mounted under /drinks in the
//! portfolio server or served standalone via the bin target.

pub mod auth;
pub mod db;
pub mod error;
pub mod hub;
pub mod mechanics;
pub mod models;
pub mod render;
pub mod rooms;
pub mod routes;

use std::sync::Arc;

/// Rooms idle longer than this are ended by the hourly sweep.
pub const MAX_IDLE_HOURS: i64 = 12;

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
    };
    spawn_cleanup(state.clone());
    mechanics::spawn_ticker(state.clone());
    routes::router().with_state(state)
}
