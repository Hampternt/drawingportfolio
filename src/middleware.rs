use crate::{
    db,
    models::{Session, UserId},
    AppState,
};
use axum::{
    extract::{ConnectInfo, FromRequestParts},
    http::{request::Parts, StatusCode},
    response::{IntoResponse, Redirect},
};
use std::{net::SocketAddr, sync::Arc};

/// Extractor: requires a valid session cookie. Redirects to /admin/login if
/// missing/expired.
///
/// This answers "who are you", **not** "are you an admin". Holding one of these
/// is enough for `/fitness` and nothing else — every art-portfolio route wants
/// [`RequireAdmin`]. Before multi-user the two questions had the same answer,
/// which is exactly the assumption this type exists to break.
pub struct AuthSession {
    pub user_id: i64,
    pub user_name: String,
    pub is_owner: bool,
    pub is_admin: bool,
}

impl AuthSession {
    /// The only admin question anything should ask.
    ///
    /// The owner is an admin without carrying the grant flag, so nothing reads
    /// `is_admin` directly.
    pub fn is_effective_admin(&self) -> bool {
        self.is_owner || self.is_admin
    }

    /// Whose data this request may touch.
    ///
    /// The single source of a [`UserId`] in the nutrition routes: handlers get
    /// theirs from the session and never from a path, query or form field, so
    /// there is no request shape that can ask for someone else's log.
    pub fn user(&self) -> UserId {
        UserId(self.user_id)
    }
}

impl From<Session> for AuthSession {
    fn from(s: Session) -> Self {
        AuthSession {
            user_id: s.user_id,
            user_name: s.user_name,
            is_owner: s.is_owner,
            is_admin: s.is_admin,
        }
    }
}

/// Loads the live session for this request, joined to its user.
///
/// One round-trip: `get_session` joins `users`, so the admin flags arrive with
/// the session rather than costing a second query on every HTMX fragment swap.
async fn load_session(parts: &Parts, state: &Arc<AppState>) -> Option<Session> {
    let id = extract_session_cookie(parts)?;
    db::get_session(&state.pool, &id).await
}

impl FromRequestParts<Arc<AppState>> for AuthSession {
    type Rejection = axum::response::Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        match load_session(parts, state).await {
            Some(session) => Ok(session.into()),
            None => {
                tracing::warn!("rejected request with no valid session");
                Err(Redirect::to("/admin/login").into_response())
            }
        }
    }
}

/// Extractor: requires a session whose user is an effective admin (the owner,
/// or someone granted the flag).
///
/// The two rejections are deliberately different:
///
/// - **No session at all** → redirect to the login page, as before.
/// - **A valid session without admin** → `404`, never a redirect and never a
///   `403`. Redirecting would bounce a signed-in member to a login screen they
///   are already past, and both a redirect and a 403 confirm the route exists.
///   This mirrors the visibility model's own rule for hidden posts.
///
/// A unit struct on purpose: no handler yet needs to know *which* admin is
/// asking. Pack 3's management page will, and widening this to carry the
/// [`AuthSession`] costs nothing at the seventeen call sites — they all bind
/// it as `_`.
pub struct RequireAdmin;

impl FromRequestParts<Arc<AppState>> for RequireAdmin {
    type Rejection = axum::response::Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        match load_session(parts, state).await {
            Some(session) => {
                let auth: AuthSession = session.into();
                if auth.is_effective_admin() {
                    Ok(RequireAdmin)
                } else {
                    tracing::warn!(
                        "non-admin user {} ({}) refused an admin route",
                        auth.user_id,
                        auth.user_name
                    );
                    Err(StatusCode::NOT_FOUND.into_response())
                }
            }
            None => {
                tracing::warn!("rejected admin request with no valid session");
                Err(Redirect::to("/admin/login").into_response())
            }
        }
    }
}

/// Extractor: requires *the* owner — the single account migration 015 seeds and
/// its partial unique index protects.
///
/// Stricter than [`RequireAdmin`] on purpose, and used only for user
/// management. An admin manages art; they cannot mint more admins, reset
/// someone's PIN or delete an account. That keeps the privilege graph acyclic:
/// no grant a member receives can ever be turned back on the owner.
///
/// Rejects exactly as `RequireAdmin` does — 404 for a valid non-owner session,
/// redirect only when there is no session at all.
pub struct RequireOwner(pub AuthSession);

impl FromRequestParts<Arc<AppState>> for RequireOwner {
    type Rejection = axum::response::Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        match load_session(parts, state).await {
            Some(session) => {
                let auth: AuthSession = session.into();
                if auth.is_owner {
                    Ok(RequireOwner(auth))
                } else {
                    tracing::warn!(
                        "non-owner user {} ({}) refused an owner-only route",
                        auth.user_id,
                        auth.user_name
                    );
                    Err(StatusCode::NOT_FOUND.into_response())
                }
            }
            None => {
                tracing::warn!("rejected owner-only request with no valid session");
                Err(Redirect::to("/admin/login").into_response())
            }
        }
    }
}

/// Extractor: checks admin status without ever rejecting. `true` means the
/// requester is an **effective admin**, not merely that they are logged in.
///
/// The distinction is the whole point of the rename. `feed.rs` turns this flag
/// straight into `Viewer::Admin` and `tasks.rs` uses it to render management
/// controls — so while this meant "has a session", the first fitness-only
/// member to log in would have been shown every unlisted and hidden art post
/// on the site.
pub struct OptionalAdmin(pub bool);

impl FromRequestParts<Arc<AppState>> for OptionalAdmin {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        let is_admin = load_session(parts, state)
            .await
            .map(|s| s.is_effective_admin())
            .unwrap_or(false);
        Ok(OptionalAdmin(is_admin))
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
        let addr = parts
            .extensions
            .get::<ConnectInfo<SocketAddr>>()
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
