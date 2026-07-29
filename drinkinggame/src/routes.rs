use askama::Template;
use axum::extract::Form;
use axum::extract::Path;
use axum::extract::State;
use axum::http::header;
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{Html, IntoResponse, Redirect};
use axum::routing::{get, post};
use axum::Router;
use futures::StreamExt;
use serde::Deserialize;
use std::convert::Infallible;
use tokio_stream::wrappers::BroadcastStream;

use crate::auth::{self, OptionalPlayer, PlayerSession};
use crate::db;
use crate::error::GameError;
use crate::hub::RoomMessage;
use crate::render;
use crate::rooms;
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

#[derive(Template)]
#[template(path = "room.html")]
struct RoomTemplate {
    base_path: String,
    code: String,
    player_name: String,
    leaderboard_items: String,
}

async fn create_room(
    State(state): State<GameState>,
    PlayerSession(player): PlayerSession,
) -> impl IntoResponse {
    let room = rooms::create_room_with_unique_code(&state.pool).await;
    db::join_room(&state.pool, room.id, player.id).await;
    Redirect::to(&format!("{}/room/{}", state.base_path, room.code))
}

#[derive(Deserialize)]
struct JoinForm {
    code: String,
}

async fn join_room_handler(
    State(state): State<GameState>,
    PlayerSession(_player): PlayerSession,
    Form(form): Form<JoinForm>,
) -> axum::response::Response {
    let code = form.code.trim().to_uppercase();
    match db::get_open_room(&state.pool, &code).await {
        // The room page itself performs the join — one code path for both
        // form joins and shared-link joins.
        Some(_) => Redirect::to(&format!("{}/room/{code}", state.base_path)).into_response(),
        None => error_page(&state, StatusCode::NOT_FOUND, "No open room with that code"),
    }
}

async fn room_page(
    State(state): State<GameState>,
    PlayerSession(player): PlayerSession,
    Path(code): Path<String>,
) -> axum::response::Response {
    let code = code.to_uppercase();
    let Some(room) = db::get_open_room(&state.pool, &code).await else {
        return error_page(
            &state,
            StatusCode::NOT_FOUND,
            "Room not found or already ended",
        );
    };
    // Visiting a room joins it: room URLs double as invite links.
    db::join_room(&state.pool, room.id, player.id).await;
    let rows = db::leaderboard(&state.pool, room.id).await;
    let tpl = RoomTemplate {
        base_path: state.base_path.to_string(),
        code,
        player_name: player.name,
        leaderboard_items: render::leaderboard_items(&rows),
    };
    Html(tpl.render().unwrap()).into_response()
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

/// Re-render the standings and push to every subscribed screen in the room.
pub(crate) async fn broadcast_leaderboard(state: &GameState, room_id: i64) {
    let rows = db::leaderboard(&state.pool, room_id).await;
    state.hub.publish(
        room_id,
        crate::hub::RoomMessage::Leaderboard(render::leaderboard_items(&rows)),
    );
}

#[derive(Deserialize)]
struct EventForm {
    kind: String,
}

async fn log_event(
    State(state): State<GameState>,
    PlayerSession(player): PlayerSession,
    Path(code): Path<String>,
    Form(form): Form<EventForm>,
) -> axum::response::Response {
    if form.kind != "drink" && form.kind != "shot" {
        return StatusCode::UNPROCESSABLE_ENTITY.into_response();
    }
    let Some(room) = db::get_open_room(&state.pool, &code.to_uppercase()).await else {
        return GameError::RoomNotFound.into_response();
    };
    if !db::is_room_member(&state.pool, room.id, player.id).await {
        return StatusCode::FORBIDDEN.into_response();
    }
    db::insert_event(&state.pool, room.id, player.id, &form.kind).await;
    db::touch_room(&state.pool, room.id).await;
    crate::mechanics::on_event(room.id, player.id, &form.kind);
    broadcast_leaderboard(&state, room.id).await;
    StatusCode::NO_CONTENT.into_response()
}

async fn undo_event(
    State(state): State<GameState>,
    PlayerSession(player): PlayerSession,
    Path(code): Path<String>,
) -> axum::response::Response {
    let Some(room) = db::get_open_room(&state.pool, &code.to_uppercase()).await else {
        return GameError::RoomNotFound.into_response();
    };
    if !db::is_room_member(&state.pool, room.id, player.id).await {
        return StatusCode::FORBIDDEN.into_response();
    }
    if db::undo_last_event(&state.pool, room.id, player.id).await {
        db::touch_room(&state.pool, room.id).await;
        broadcast_leaderboard(&state, room.id).await;
    }
    StatusCode::NO_CONTENT.into_response()
}

async fn end_room_handler(
    State(state): State<GameState>,
    PlayerSession(player): PlayerSession,
    Path(code): Path<String>,
) -> axum::response::Response {
    let Some(room) = db::get_open_room(&state.pool, &code.to_uppercase()).await else {
        return GameError::RoomNotFound.into_response();
    };
    if !db::is_room_member(&state.pool, room.id, player.id).await {
        return StatusCode::FORBIDDEN.into_response();
    }
    db::end_room(&state.pool, room.id).await;
    state.hub.publish(room.id, crate::hub::RoomMessage::Ended);
    state.hub.remove(room.id);
    Redirect::to(&format!("{}/", state.base_path)).into_response()
}

async fn sse_stream(
    State(state): State<GameState>,
    Path(code): Path<String>,
) -> axum::response::Response {
    let Some(room) = db::get_open_room(&state.pool, &code.to_uppercase()).await else {
        return GameError::RoomNotFound.into_response();
    };

    // Subscribe BEFORE rendering the snapshot — no update can slip between.
    let rx = state.hub.subscribe(room.id);
    // Re-check after subscribing: if the room was ended in the window between
    // the lookup and the subscribe, the subscribe above resurrected a channel
    // entry that the end path already removed — drop it again and tell the
    // client the night is over instead of leaking a zombie hub entry.
    if db::get_open_room(&state.pool, &room.code).await.is_none() {
        state.hub.remove(room.id);
        let stream = futures::stream::once(async move {
            Ok::<_, Infallible>(Event::default().event("ended").data(""))
        });
        return (
            [(header::HeaderName::from_static("x-accel-buffering"), "no")],
            Sse::new(stream).keep_alive(KeepAlive::default()),
        )
            .into_response();
    }
    let rows = db::leaderboard(&state.pool, room.id).await;
    let initial = render::leaderboard_items(&rows);

    let stream = futures::stream::once(async move {
        Ok::<_, Infallible>(Event::default().event("leaderboard").data(initial))
    })
    .chain(BroadcastStream::new(rx).filter_map(|msg| async move {
        match msg {
            Ok(RoomMessage::Leaderboard(html)) => {
                Some(Ok(Event::default().event("leaderboard").data(html)))
            }
            Ok(RoomMessage::Game(html)) => Some(Ok(Event::default().event("game").data(html))),
            Ok(RoomMessage::Ended) => Some(Ok(Event::default().event("ended").data(""))),
            // Lagged receiver: skip — the next update carries full state anyway.
            Err(_) => None,
        }
    }));

    (
        // Belt-and-braces alongside nginx's proxy_buffering off.
        [(header::HeaderName::from_static("x-accel-buffering"), "no")],
        Sse::new(stream).keep_alive(KeepAlive::default()),
    )
        .into_response()
}

#[derive(Template)]
#[template(path = "screen.html")]
struct ScreenTemplate {
    base_path: String,
    code: String,
    leaderboard_items: String,
}

async fn screen_page(
    State(state): State<GameState>,
    Path(code): Path<String>,
) -> axum::response::Response {
    let code = code.to_uppercase();
    let Some(room) = db::get_open_room(&state.pool, &code).await else {
        return error_page(
            &state,
            StatusCode::NOT_FOUND,
            "Room not found or already ended",
        );
    };
    let rows = db::leaderboard(&state.pool, room.id).await;
    let tpl = ScreenTemplate {
        base_path: state.base_path.to_string(),
        code,
        leaderboard_items: render::leaderboard_items(&rows),
    };
    Html(tpl.render().unwrap()).into_response()
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
        .route("/rooms", post(create_room))
        .route("/join", post(join_room_handler))
        .route("/room/{code}", get(room_page))
        .route("/room/{code}/event", post(log_event))
        .route("/room/{code}/undo", post(undo_event))
        .route("/room/{code}/end", post(end_room_handler))
        .route("/room/{code}/sse", get(sse_stream))
        .route("/room/{code}/screen", get(screen_page))
        .route("/assets/game.css", get(game_css))
        .route("/assets/htmx.min.js", get(htmx_js))
}
