//! # Drawing Portfolio — Application Entry Point
//!
//! This file is the heart of the program. When you run `cargo run`, Rust
//! starts here. It does three things in order:
//!   1. Builds the shared application state (database, file storage, auth engine)
//!   2. Registers all URL routes
//!   3. Starts the HTTP server and blocks forever, handling requests

// --- Module declarations ---
// These tell Rust "there are other .rs files in src/ that belong to this project".
// Each `mod` declaration makes the module's public items available under that name,
// e.g. `db::connect(...)` or `routes::feed::router()`.
mod db;         // All database queries (SQLite via sqlx)
mod middleware; // Custom request extractors: AuthSession, OptionalAuth, LocalhostOnly
mod models;     // Plain data structs that mirror database rows
mod routes;     // One sub-module per feature area (hub, feed, admin, auth, nutrition)
mod storage;    // S3-compatible object storage wrapper (Hetzner)

use std::sync::Arc;
use axum::Router;
use axum::extract::DefaultBodyLimit;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use webauthn_rs::prelude::*;
use url::Url;

/// Shared application state, passed to every route handler.
///
/// Wrapped in `Arc<AppState>` so it can be cheaply cloned across threads
/// without copying the actual data. `Arc` = Atomic Reference Count — it keeps
/// track of how many owners there are and drops the data when the count hits zero.
///
/// All three fields are designed to be shared across threads (they implement
/// `Send + Sync`), so no mutex is needed here.
pub struct AppState {
    /// SQLite connection pool. sqlx manages a pool of connections so multiple
    /// requests can query the database at the same time without blocking.
    pub pool: db::DbPool,

    /// Wraps the S3-compatible client for uploading/deleting images.
    pub storage: storage::ObjectStorage,

    /// The WebAuthn engine used to verify passkey login ceremonies.
    pub webauthn: Webauthn,
}

/// `#[tokio::main]` is a macro that wraps `main()` so it runs inside Tokio's
/// async runtime. Without this, `async fn main()` wouldn't compile — Rust has
/// no built-in async runtime, so you must bring one (Tokio is the most common).
#[tokio::main]
async fn main() {
    // Set up structured logging. After this, `tracing::info!(...)` etc. print
    // formatted log lines to stdout. Controlled by the RUST_LOG env variable.
    tracing_subscriber::fmt::init();

    // Load environment variables from the `.env` file into the process.
    // `.ok()` discards the error — if there's no .env file (e.g. in production
    // where env vars are set by systemd), that's fine.
    dotenvy::dotenv().ok();

    // --- Database setup ---
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:./portfolio.db".to_string());

    // `await` suspends this async function until the connection pool is ready.
    // Rust's async model: `.await` yields control back to the Tokio runtime,
    // which can run other tasks while waiting — no thread is blocked.
    let pool = db::connect(&database_url).await;

    // Run any pending database migrations (CREATE TABLE IF NOT EXISTS, etc.).
    // These are defined in db.rs and are idempotent — safe to run on every startup.
    db::run_migrations(&pool).await;

    // --- Object storage setup ---
    // Reads AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY, STORAGE_ENDPOINT etc. from env.
    let storage = storage::ObjectStorage::from_env().await;

    // --- WebAuthn (passkey) setup ---
    // RP = "Relying Party" — WebAuthn's term for your website.
    // RP_ID must be the bare domain (e.g. "example.com").
    // RP_ORIGIN must be the full origin with scheme (e.g. "https://example.com").
    let rp_id = std::env::var("RP_ID").unwrap_or_else(|_| "localhost".to_string());
    let rp_origin = std::env::var("RP_ORIGIN")
        .unwrap_or_else(|_| "http://localhost:3000".to_string());
    let rp_origin_url = Url::parse(&rp_origin).expect("invalid RP_ORIGIN");

    let webauthn = WebauthnBuilder::new(&rp_id, &rp_origin_url)
        .expect("invalid WebAuthn config")
        .rp_name("Drawing Portfolio")
        .build()
        .expect("failed to build WebAuthn");

    // Wrap AppState in Arc so it can be shared across every route handler.
    // From this point on, `state` is never mutated — it's read-only shared data.
    let state = Arc::new(AppState { pool, storage, webauthn });

    // --- Background cleanup task ---
    // `tokio::spawn` launches a new async task that runs concurrently alongside
    // the HTTP server. It's fire-and-forget — we don't await it.
    //
    // This task deletes expired sessions and WebAuthn challenges once per hour,
    // keeping the database tidy without needing a cron job or external scheduler.
    {
        // Clone the pool *before* moving into the async block.
        // `Arc::clone` is cheap — it just increments the reference count.
        // The `state` Arc owns one copy; this block owns a second copy.
        let pool = state.pool.clone();
        tokio::spawn(async move {
            // `interval` fires immediately, then every 3600 seconds (1 hour).
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(3600));
            loop {
                // `.tick().await` yields until the next interval fires.
                interval.tick().await;
                db::cleanup_expired(&pool).await;
            }
        });
    } // ← the extra scope here is just style: it drops the `pool` clone
      //   from this scope immediately, making the ownership transfer obvious.

    // --- Route registration ---
    // Each route module exposes a `router()` function returning an Axum `Router`.
    // `.merge()` combines them into one big router — order doesn't matter.
    // `.with_state(state)` injects AppState into every handler that asks for it.
    let app = Router::new()
        .merge(routes::hub::router())        // GET /
        .merge(routes::feed::router())       // GET /artportfolio (and HTMX/JSON sub-routes)
        .merge(routes::admin::router())      // GET /admin, POST/DELETE /api/admin/posts
        .merge(routes::auth::router())       // POST /api/auth/... (WebAuthn ceremonies)
        .merge(routes::nutrition::router())  // GET /fitness, POST/DELETE /api/nutrition/...
        // Serve files from the `static/` directory on disk at the /static URL prefix.
        // Unlike templates (compiled into the binary), these are read from disk at runtime.
        .nest_service("/static", ServeDir::new("static"))
        // Cap incoming request bodies at 35 MB — must match nginx's client_max_body_size.
        .layer(DefaultBodyLimit::max(35 * 1024 * 1024))
        // Log every HTTP request/response (method, path, status, latency).
        .layer(TraceLayer::new_for_http())
        .with_state(state)
        // If no route matched, return a plain 404.
        .fallback(handler_404);

    // Bind to all network interfaces on port 3000.
    // nginx sits in front and forwards traffic here (only from localhost).
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    tracing::info!("listening on {}", listener.local_addr().unwrap());

    // `into_make_service_with_connect_info` makes the client's socket address
    // available to handlers — needed by the `LocalhostOnly` extractor in middleware.rs.
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    ).await.unwrap();
}

/// Fallback handler for any URL that doesn't match a registered route.
/// Returns HTTP 404 with a plain text body.
///
/// `impl IntoResponse` means "any type that can be turned into an HTTP response" —
/// Axum knows how to convert a `(StatusCode, &str)` tuple automatically.
async fn handler_404() -> impl axum::response::IntoResponse {
    (axum::http::StatusCode::NOT_FOUND, "not found")
}
