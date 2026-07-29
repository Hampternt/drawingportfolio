use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};

/// Typed domain errors. IntoResponse renders an HTML fragment suitable for
/// an HTMX swap; full-page errors (e.g. visiting a dead room URL) are
/// rendered by the route handlers themselves, which know base_path.
#[derive(thiserror::Error, Debug)]
pub enum GameError {
    #[error("name must be 1–20 characters")]
    InvalidName,
    #[error("PIN must be exactly 4 digits")]
    InvalidPin,
    #[error("wrong PIN for that name")]
    WrongPin,
    #[error("room not found or already ended")]
    RoomNotFound,
    #[error("no Ring of Fire game is running")]
    NoActiveGame,
    #[error("a game is already running in this room")]
    GameAlreadyActive,
    #[error("the deck is empty")]
    DeckExhausted,
    #[error("you don't hold that card")]
    CardNotHeld,
    #[error("no preset with that id")]
    PresetNotFound,
    #[error("something went wrong, try again")]
    Db(#[from] sqlx::Error),
}

impl IntoResponse for GameError {
    fn into_response(self) -> Response {
        let status = match &self {
            GameError::InvalidName | GameError::InvalidPin => StatusCode::UNPROCESSABLE_ENTITY,
            GameError::WrongPin => StatusCode::UNAUTHORIZED,
            GameError::NoActiveGame | GameError::PresetNotFound => StatusCode::NOT_FOUND,
            GameError::GameAlreadyActive | GameError::DeckExhausted => StatusCode::CONFLICT,
            GameError::CardNotHeld => StatusCode::FORBIDDEN,
            GameError::RoomNotFound => StatusCode::NOT_FOUND,
            GameError::Db(e) => {
                tracing::error!("db error: {e}");
                StatusCode::INTERNAL_SERVER_ERROR
            }
        };
        let body = format!(
            r#"<p class="error">{}</p>"#,
            crate::render::html_escape(&self.to_string())
        );
        (
            status,
            [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
            body,
        )
            .into_response()
    }
}
