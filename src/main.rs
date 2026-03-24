mod db;
mod middleware;
mod models;
mod routes;
mod storage;

use std::sync::Arc;
use axum::Router;
use tower_http::services::ServeDir;

pub struct AppState {
    pub pool: db::DbPool,
    pub storage: storage::R2Storage,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    dotenvy::dotenv().ok();

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:./portfolio.db".to_string());

    let pool = db::connect(&database_url).await;
    db::run_migrations(&pool).await;

    let storage = storage::R2Storage::from_env().await;

    let state = Arc::new(AppState { pool, storage });

    {
        let pool = state.pool.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(3600));
            loop {
                interval.tick().await;
                db::cleanup_expired(&pool).await;
            }
        });
    }

    let app = Router::new()
        .merge(routes::feed::router())
        .merge(routes::admin::router())
        .merge(routes::auth::router())
        .nest_service("/static", ServeDir::new("static"))
        .with_state(state)
        .fallback(handler_404);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    tracing::info!("listening on {}", listener.local_addr().unwrap());
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    ).await.unwrap();
}

async fn handler_404() -> impl axum::response::IntoResponse {
    (axum::http::StatusCode::NOT_FOUND, "not found")
}
