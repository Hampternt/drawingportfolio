use askama::Template;
use axum::extract::Form;
use axum::extract::State;
use axum::http::header;
use axum::response::{Html, IntoResponse, Redirect};
use axum::routing::{get, post};
use axum::Router;
use serde::Deserialize;

use crate::auth::{self, OptionalPlayer};
use crate::db;
use crate::error::GameError;
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

async fn landing(
    State(state): State<GameState>,
    OptionalPlayer(player): OptionalPlayer,
) -> impl IntoResponse {
    let tpl = match player {
        Some(p) => {
            let (drinks, shots) = db::lifetime_counts(&state.pool, p.id).await;
            LandingTemplate {
                base_path: state.base_path.to_string(),
                logged_in: true,
                player_name: p.name,
                lifetime_drinks: drinks,
                lifetime_shots: shots,
            }
        }
        None => LandingTemplate {
            base_path: state.base_path.to_string(),
            logged_in: false,
            player_name: String::new(),
            lifetime_drinks: 0,
            lifetime_shots: 0,
        },
    };
    Html(tpl.render().unwrap())
}

#[derive(Deserialize)]
struct LoginForm {
    name: String,
    pin: String,
}

async fn login(
    State(state): State<GameState>,
    Form(form): Form<LoginForm>,
) -> axum::response::Response {
    match auth::login_or_register(&state.pool, &form.name, &form.pin).await {
        Ok(player) => {
            let sid = auth::new_session_id();
            db::create_session(&state.pool, &sid, player.id, "+90 days").await;
            (
                [(header::SET_COOKIE, auth::session_cookie(&sid))],
                Redirect::to(&format!("{}/", state.base_path)),
            )
                .into_response()
        }
        // The login form is a plain (non-HTMX) post, so render the friendly
        // full error page — a bare fragment would arrive unstyled.
        Err(e @ GameError::WrongPin) => {
            error_page(&state, axum::http::StatusCode::UNAUTHORIZED, &e.to_string())
        }
        Err(e @ (GameError::InvalidName | GameError::InvalidPin)) => error_page(
            &state,
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            &e.to_string(),
        ),
        Err(e) => e.into_response(),
    }
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
        .route("/login", post(login))
        .route("/assets/game.css", get(game_css))
        .route("/assets/htmx.min.js", get(htmx_js))
}
