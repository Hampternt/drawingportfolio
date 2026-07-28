//! Handler-level integration tests via tower::ServiceExt, portfolio-style.

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use sqlx::sqlite::SqlitePoolOptions;
use tower::ServiceExt;

async fn test_app() -> Router {
    // max_connections(1): a :memory: db exists per-connection.
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    drinkinggame::db::run_migrations(&pool).await;
    drinkinggame::router_with_pool(pool, "")
}

async fn body_string(res: axum::response::Response) -> String {
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    String::from_utf8(bytes.to_vec()).unwrap()
}

#[tokio::test]
async fn test_landing_serves_login_form() {
    let app = test_app().await;
    let res = app
        .oneshot(Request::get("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let html = body_string(res).await;
    assert!(html.contains(r#"action="/login""#));
    assert!(html.contains("4-digit PIN"));
}

#[tokio::test]
async fn test_assets_are_served() {
    let app = test_app().await;
    for (path, ct) in [
        ("/assets/game.css", "text/css"),
        ("/assets/htmx.min.js", "application/javascript"),
    ] {
        let res = app
            .clone()
            .oneshot(Request::get(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK, "{path}");
        assert_eq!(res.headers()[header::CONTENT_TYPE], ct, "{path}");
    }
}
