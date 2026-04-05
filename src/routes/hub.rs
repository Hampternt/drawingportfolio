use axum::{Router, routing::get, response::{Html, IntoResponse}};
use askama::Template;
use std::sync::Arc;
use crate::{AppState, middleware::OptionalAuth};

#[derive(Template)]
#[template(path = "hub/hub.html")]
struct HubTemplate {
    is_admin: bool,
}

async fn hub_page(
    OptionalAuth(is_admin): OptionalAuth,
) -> impl IntoResponse {
    Html(HubTemplate { is_admin }.render().unwrap())
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(hub_page))
}
