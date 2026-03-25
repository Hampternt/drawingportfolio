use axum::{
    Router,
    routing::post,
    extract::State,
    response::IntoResponse,
    http::{HeaderMap, StatusCode},
};
use std::sync::Arc;
use crate::AppState;

async fn logout(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if let Some(cookies) = headers.get("cookie").and_then(|v| v.to_str().ok()) {
        for cookie in cookies.split(';') {
            if let Some(id) = cookie.trim().strip_prefix("session=") {
                crate::db::delete_session(&state.pool, id).await;
            }
        }
    }
    let mut response_headers = axum::http::HeaderMap::new();
    response_headers.insert(
        axum::http::header::SET_COOKIE,
        "session=; HttpOnly; SameSite=Strict; Max-Age=0; Path=/".parse().unwrap(),
    );
    response_headers.insert(
        axum::http::header::LOCATION,
        "/admin/login".parse().unwrap(),
    );
    (StatusCode::SEE_OTHER, response_headers).into_response()
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/auth/logout", post(logout))
        // WebAuthn routes added in Task 7
}
