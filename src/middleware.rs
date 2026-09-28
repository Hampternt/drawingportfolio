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

/// Extractor: requires a valid session cookie. Sends the request to
/// /admin/login if missing/expired (see [`to_login`]).
///
/// This answers "who are you", **not** "are you an admin". Holding one of these
/// is enough for `/fitness`, `/sorting` and the self-service account routes,
/// and nothing else — every art-portfolio route wants [`RequireAdmin`]. Before multi-user the two questions had the same answer,
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

/// What every session extractor answers when there is no session at all.
///
/// A page load gets a redirect to the login page. An htmx request gets
/// `HX-Redirect` instead, which makes htmx navigate the whole window. A boosted
/// click would otherwise follow the redirect inside its XHR and swap
/// `login.html` into the current page — without its `<head>`, so unstyled, and
/// with `#pin-form` boosted, which htmx 2.0.4 submits as a GET carrying the
/// name and PIN in the URL even after `startPinLogin` has prevented the
/// default. The 401 keeps a rejected request from ever reading as a 200.
fn to_login(parts: &Parts) -> axum::response::Response {
    if parts.headers.contains_key("hx-request") {
        (StatusCode::UNAUTHORIZED, [("HX-Redirect", "/admin/login")]).into_response()
    } else {
        Redirect::to("/admin/login").into_response()
    }
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
                Err(to_login(parts))
            }
        }
    }
}

/// Extractor: requires a session whose user is an effective admin (the owner,
/// or someone granted the flag).
///
/// The two rejections are deliberately different:
///
/// - **No session at all** → the login page, via [`to_login`].
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
                Err(to_login(parts))
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
/// A unit struct for the same reason [`RequireAdmin`] is: the management
/// handlers act on a user id from the path, never on the requester's own
/// identity, so none of them needs to know *which* owner is asking — and there
/// is only ever one.
pub struct RequireOwner;

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
                    Ok(RequireOwner)
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
                Err(to_login(parts))
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request, routing::get, Router};
    use sqlx::sqlite::SqlitePoolOptions;
    use tower::ServiceExt;

    /// One route per session extractor, so each one's no-session answer is
    /// checked on its own rather than through whichever handler uses it.
    async fn app() -> Router {
        let pool = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .unwrap();
        crate::db::run_migrations(&pool).await;
        let storage = crate::storage::ObjectStorage::from_env().await;
        let rp_origin = url::Url::parse("http://localhost:3000").unwrap();
        let webauthn = webauthn_rs::prelude::WebauthnBuilder::new("localhost", &rp_origin)
            .unwrap()
            .build()
            .unwrap();
        let state = Arc::new(AppState {
            pool,
            storage,
            webauthn,
        });
        Router::new()
            .route("/session", get(|_: AuthSession| async { "ok" }))
            .route("/admin", get(|_: RequireAdmin| async { "ok" }))
            .route("/owner", get(|_: RequireOwner| async { "ok" }))
            .with_state(state)
    }

    fn get_req(uri: &str, htmx: bool) -> Request<Body> {
        let mut b = Request::builder().method("GET").uri(uri);
        if htmx {
            b = b.header("HX-Request", "true");
        }
        b.body(Body::empty()).unwrap()
    }

    #[tokio::test]
    async fn test_no_session_page_load_redirects_to_login() {
        let app = app().await;
        for uri in ["/session", "/admin", "/owner"] {
            let resp = app.clone().oneshot(get_req(uri, false)).await.unwrap();
            assert_eq!(resp.status(), StatusCode::SEE_OTHER, "{uri}");
            assert_eq!(resp.headers()["location"], "/admin/login", "{uri}");
            assert!(!resp.headers().contains_key("hx-redirect"), "{uri}");
        }
    }

    /// The boosted-login bug: a 302/303 here is followed inside htmx's XHR, and
    /// the login page lands in the current page with its PIN form boosted into
    /// a GET. `HX-Redirect` with no `Location` is what makes htmx navigate.
    #[tokio::test]
    async fn test_no_session_htmx_request_navigates_to_login() {
        let app = app().await;
        for uri in ["/session", "/admin", "/owner"] {
            let resp = app.clone().oneshot(get_req(uri, true)).await.unwrap();
            assert_eq!(resp.status(), StatusCode::UNAUTHORIZED, "{uri}");
            assert_eq!(resp.headers()["hx-redirect"], "/admin/login", "{uri}");
            assert!(
                !resp.headers().contains_key("location"),
                "{uri}: a Location header would be followed inside the XHR"
            );
        }
    }
}
