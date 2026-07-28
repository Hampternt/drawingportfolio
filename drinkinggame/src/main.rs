//! Standalone dev server: `cargo run -p drinkinggame` then open
//! http://localhost:3001 — no portfolio, no nginx, base_path is "".

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    dotenvy::dotenv().ok();

    let config = drinkinggame::Config {
        database_url: std::env::var("DRINKS_DATABASE_URL")
            .unwrap_or_else(|_| "sqlite:./drinkinggame.db".to_string()),
        base_path: String::new(),
    };
    let app = drinkinggame::router(config).await;

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3001").await.unwrap();
    tracing::info!(
        "drinkinggame standalone on {}",
        listener.local_addr().unwrap()
    );
    axum::serve(listener, app).await.unwrap();
}
