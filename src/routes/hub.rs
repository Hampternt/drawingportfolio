use axum::{Router, routing::get, response::{Html, IntoResponse}};
use askama::Template;
use std::sync::Arc;
use crate::AppState;

#[derive(Template)]
#[template(path = "hub/hub.html")]
struct HubTemplate;

async fn hub_page() -> impl IntoResponse {
    Html(HubTemplate.render().unwrap())
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(hub_page))
}
