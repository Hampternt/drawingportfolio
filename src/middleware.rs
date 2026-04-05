use axum::{
    extract::{FromRequestParts, ConnectInfo},
    http::{request::Parts, StatusCode},
    response::{IntoResponse, Redirect},
};
use std::{net::SocketAddr, sync::Arc};
use crate::{AppState, db};

/// Extractor: requires a valid session cookie. Redirects to /admin/login if missing/expired.
pub struct AuthSession(pub String); // session ID

impl FromRequestParts<Arc<AppState>> for AuthSession {
    type Rejection = axum::response::Response;

    async fn from_request_parts(parts: &mut Parts, state: &Arc<AppState>) -> Result<Self, Self::Rejection> {
        let session_id = extract_session_cookie(parts);

        if let Some(id) = session_id {
            if db::get_session(&state.pool, &id).await.is_some() {
                return Ok(AuthSession(id));
            }
            tracing::warn!("rejected expired/invalid session");
        } else {
            tracing::warn!("rejected request with no session cookie");
        }

        Err(Redirect::to("/admin/login").into_response())
    }
}

pub fn extract_session_cookie(parts: &Parts) -> Option<String> {
    let cookies = parts.headers.get("cookie")?.to_str().ok()?;
    for cookie in cookies.split(';') {
        let cookie = cookie.trim();
        if let Some(val) = cookie.strip_prefix("session=") {
            return Some(val.to_string());
        }
    }
    None
}

/// Extractor: only allows requests from localhost (raw socket address).
pub struct LocalhostOnly;

impl<S: Send + Sync> FromRequestParts<S> for LocalhostOnly {
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let addr = parts.extensions.get::<ConnectInfo<SocketAddr>>()
            .map(|ci| ci.0);

        match addr {
            Some(addr) if addr.ip().is_loopback() => Ok(LocalhostOnly),
            _ => Err(StatusCode::FORBIDDEN),
        }
    }
}

pub fn make_session_cookie(id: &str) -> String {
    format!("session={id}; HttpOnly; SameSite=Strict; Max-Age=2592000; Path=/")
}
