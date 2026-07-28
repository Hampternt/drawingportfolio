use askama::Template;
use axum::extract::State;
use axum::http::header;
use axum::response::{Html, IntoResponse};
use axum::routing::get;
use axum::Router;

use crate::GameState;

#[derive(Template)]
#[template(path = "landing.html")]
struct LandingTemplate {
    base_path: String,
    logged_in: bool,
    player_name: String,
    lifetime_drinks: i64,
    lifetime_shots: i64,
}

#[derive(Template)]
#[template(path = "error.html")]
pub struct ErrorTemplate {
    base_path: String,
    message: String,
}

/// Full-page friendly error (unknown/ended room, etc.) with a link home.
pub fn error_page(
    state: &GameState,
    status: axum::http::StatusCode,
    message: &str,
) -> axum::response::Response {
    let tpl = ErrorTemplate {
        base_path: state.base_path.to_string(),
        message: message.to_string(),
    };
    (status, Html(tpl.render().unwrap())).into_response()
}

async fn landing(State(state): State<GameState>) -> impl IntoResponse {
    // Task 7 threads the session through; for now always the logged-out view.
    let tpl = LandingTemplate {
        base_path: state.base_path.to_string(),
        logged_in: false,
        player_name: String::new(),
        lifetime_drinks: 0,
        lifetime_shots: 0,
    };
    Html(tpl.render().unwrap())
}

async fn game_css() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css")],
        include_str!("../assets/game.css"),
    )
}

/// Single htmx copy for the whole repo: embed the portfolio's vendored file.
async fn htmx_js() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "application/javascript")],
        include_str!("../../static/htmx.min.js"),
    )
}

pub fn router() -> Router<GameState> {
    Router::new()
        .route("/", get(landing))
        .route("/assets/game.css", get(game_css))
        .route("/assets/htmx.min.js", get(htmx_js))
}
