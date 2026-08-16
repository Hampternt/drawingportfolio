//! Owner-only account management: `/admin/users`.
//!
//! Every route here is behind [`RequireOwner`], not `RequireAdmin`. A granted
//! admin manages the art portfolio; they cannot create accounts, reset PINs,
//! grant admin or delete anybody. That keeps the privilege graph acyclic — no
//! grant the owner hands out can ever be turned back on them.
//!
//! The owner invariants are enforced twice over, and the pair is deliberate:
//! `db.rs` carries `AND is_owner = 0` on every destructive statement, and the
//! template simply omits the controls. The template is the courtesy; the SQL is
//! the rule, and it is what still holds against a hand-made request.

use crate::{middleware::RequireOwner, pin, AppState};
use askama::Template;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::{get, post},
    Form, Router,
};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Template)]
#[template(path = "users.html")]
struct UsersTemplate {
    /// `base.html` reads this for its `IS_ADMIN` script constant. Always true
    /// here — the page is owner-only.
    is_admin: bool,
    users: Vec<crate::models::UserRow>,
    error: String,
    notice: String,
}

async fn render(state: &Arc<AppState>, error: &str, notice: &str) -> axum::response::Response {
    let users = crate::db::list_users(&state.pool).await;
    Html(
        UsersTemplate {
            is_admin: true,
            users,
            error: error.to_string(),
            notice: notice.to_string(),
        }
        .render()
        .unwrap(),
    )
    .into_response()
}

async fn users_page(_: RequireOwner, State(state): State<Arc<AppState>>) -> impl IntoResponse {
    render(&state, "", "").await
}

#[derive(Deserialize)]
struct CreateForm {
    name: String,
    pin: String,
}

/// Creates a member account with an initial PIN the owner hands over.
///
/// There is no public sign-up: accounts exist because the owner made one. That
/// is the whole reason this page can be a plain form with no email
/// verification, no invite tokens and no rate limiting of its own.
async fn create_user(
    _: RequireOwner,
    State(state): State<Arc<AppState>>,
    Form(form): Form<CreateForm>,
) -> impl IntoResponse {
    let name = match pin::validate_name(&form.name) {
        Ok(n) => n,
        Err(e) => return render(&state, e.message(), "").await,
    };
    if let Err(e) = pin::validate_pin(&form.pin) {
        return render(&state, e.message(), "").await;
    }

    let hash = pin::hash_pin_async(&form.pin).await;
    match crate::db::create_user(&state.pool, &name, &hash).await {
        Ok(id) => {
            tracing::info!("owner created user {id} ({name})");
            render(
                &state,
                "",
                &format!("Created {name}. Give them that PIN — it is not shown again."),
            )
            .await
        }
        // The only realistic failure is the UNIQUE COLLATE NOCASE name index.
        Err(e) => {
            tracing::warn!("create user failed: {e}");
            render(&state, pin::CredentialError::NameTaken.message(), "").await
        }
    }
}

#[derive(Deserialize)]
struct PinForm {
    pin: String,
}

/// Resets a member's PIN — the "they forgot it" path, and the way a locked
/// account gets unlocked, since setting a PIN clears the failure counter.
async fn reset_pin(
    _: RequireOwner,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Form(form): Form<PinForm>,
) -> impl IntoResponse {
    if let Err(e) = pin::validate_pin(&form.pin) {
        return render(&state, e.message(), "").await;
    }
    let hash = pin::hash_pin_async(&form.pin).await;
    if crate::db::set_user_pin(&state.pool, id, &hash).await {
        tracing::info!("owner reset PIN for user {id}");
        render(&state, "", "PIN reset, and any lockout cleared.").await
    } else {
        render(&state, "No such user.", "").await
    }
}

#[derive(Deserialize)]
struct AdminForm {
    grant: String,
}

async fn set_admin(
    _: RequireOwner,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Form(form): Form<AdminForm>,
) -> impl IntoResponse {
    let grant = form.grant == "1";
    if crate::db::set_user_admin(&state.pool, id, grant).await {
        tracing::info!("owner set is_admin={grant} on user {id}");
        let what = if grant { "granted" } else { "revoked" };
        render(&state, "", &format!("Admin {what}.")).await
    } else {
        // Either no such row, or it is the owner's — `set_user_admin` carries
        // `AND is_owner = 0`, so the owner's own flag is untouchable here.
        render(&state, "That account's admin flag cannot be changed.", "").await
    }
}

async fn delete_user(
    _: RequireOwner,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    if crate::db::delete_user(&state.pool, id).await {
        tracing::info!("owner deleted user {id} and all their data");
        render(&state, "", "Account deleted, along with all of its data.").await
    } else {
        render(&state, "That account cannot be deleted.", "").await
    }
}

/// A member's own account page: change your PIN, nothing else.
///
/// Requires the *current* PIN even though the session already proves identity.
/// The session is a cookie on one device; the PIN is what logs you in from
/// another. Letting a borrowed unlocked phone silently re-key the account is
/// the failure this prevents.
#[derive(Template)]
#[template(path = "account.html")]
struct AccountTemplate {
    /// For `base.html`'s `IS_ADMIN` constant — the real one this time, since a
    /// plain member reaches this page.
    is_admin: bool,
    user_name: String,
    error: String,
    notice: String,
    has_pin: bool,
}

#[derive(Deserialize)]
struct ChangePinForm {
    current_pin: String,
    new_pin: String,
}

async fn account_page(
    session: crate::middleware::AuthSession,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    account_render(&state, &session, "", "").await
}

async fn account_render(
    state: &Arc<AppState>,
    session: &crate::middleware::AuthSession,
    error: &str,
    notice: &str,
) -> axum::response::Response {
    let has_pin = crate::db::get_user_by_name(&state.pool, &session.user_name)
        .await
        .map(|u| !u.pin_hash.is_empty())
        .unwrap_or(false);
    Html(
        AccountTemplate {
            is_admin: session.is_effective_admin(),
            user_name: session.user_name.clone(),
            error: error.to_string(),
            notice: notice.to_string(),
            has_pin,
        }
        .render()
        .unwrap(),
    )
    .into_response()
}

async fn change_own_pin(
    session: crate::middleware::AuthSession,
    State(state): State<Arc<AppState>>,
    Form(form): Form<ChangePinForm>,
) -> impl IntoResponse {
    let Some(user) = crate::db::get_user_by_name(&state.pool, &session.user_name).await else {
        return StatusCode::NOT_FOUND.into_response();
    };

    if user.is_locked {
        return account_render(&state, &session, pin::CredentialError::Locked.message(), "").await;
    }

    // The current PIN is checked through the same lockout path as login, so
    // this form cannot be used as an unmetered oracle for guessing it.
    if !pin::verify_pin_async(&form.current_pin, &user.pin_hash).await {
        crate::db::record_failed_pin(&state.pool, user.id).await;
        return account_render(&state, &session, "Current PIN is wrong.", "").await;
    }
    if let Err(e) = pin::validate_pin(&form.new_pin) {
        return account_render(&state, &session, e.message(), "").await;
    }

    let hash = pin::hash_pin_async(&form.new_pin).await;
    crate::db::set_user_pin(&state.pool, user.id, &hash).await;
    tracing::info!("user {} changed their own PIN", user.id);
    account_render(&state, &session, "", "PIN changed.").await
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        // Owner-only account management.
        .route("/admin/users", get(users_page).post(create_user))
        .route("/api/admin/users/{id}/pin", post(reset_pin))
        .route("/api/admin/users/{id}/admin", post(set_admin))
        .route("/api/admin/users/{id}/delete", post(delete_user))
        // Any signed-in user, for their own account.
        .route("/fitness/account", get(account_page))
        .route("/api/account/pin", post(change_own_pin))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode as HttpStatus};
    use sqlx::sqlite::SqlitePoolOptions;
    use tower::ServiceExt;

    async fn app_with_pool() -> (Router, crate::db::DbPool) {
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
        let state = Arc::new(crate::AppState {
            pool: pool.clone(),
            storage,
            webauthn,
        });
        (router().with_state(state), pool)
    }

    /// A session cookie for the owner.
    async fn owner_cookie(pool: &crate::db::DbPool) -> String {
        let id = crate::db::get_owner_user_id(pool).await.unwrap();
        crate::db::create_session(pool, "owner-sess", "2099-01-01 00:00:00", id).await;
        "session=owner-sess".to_string()
    }

    /// A session cookie for a member, optionally one granted art admin.
    async fn member_cookie(pool: &crate::db::DbPool, name: &str, admin: bool) -> String {
        let id = crate::db::create_user(pool, name, "hash").await.unwrap();
        if admin {
            crate::db::set_user_admin(pool, id, true).await;
        }
        let sess = format!("{name}-sess");
        crate::db::create_session(pool, &sess, "2099-01-01 00:00:00", id).await;
        format!("session={sess}")
    }

    async fn req(method: &str, uri: &str, cookie: Option<&str>, body: &str) -> Request<Body> {
        let mut b = Request::builder()
            .method(method)
            .uri(uri)
            .header("content-type", "application/x-www-form-urlencoded");
        if let Some(c) = cookie {
            b = b.header("cookie", c);
        }
        b.body(Body::from(body.to_string())).unwrap()
    }

    /// **The pack 3 gate.** A *granted admin* is still refused user management.
    ///
    /// This is the distinction `RequireOwner` exists for, and the one a reader
    /// is most likely to assume away: an admin who could reach these routes
    /// could grant admin to anyone, reset the owner's collaborators' PINs, and
    /// delete accounts. The privilege graph has to stay acyclic — nothing the
    /// owner hands out may be turned back on them.
    #[tokio::test]
    async fn test_admin_is_still_refused_user_management() {
        let (app, pool) = app_with_pool().await;
        let admin = member_cookie(&pool, "alex", true).await;
        let plain = member_cookie(&pool, "sam", false).await;

        for cookie in [&admin, &plain] {
            for (method, uri, body) in [
                ("GET", "/admin/users", ""),
                ("POST", "/admin/users", "name=mallory&pin=1234"),
                ("POST", "/api/admin/users/2/pin", "pin=9999"),
                ("POST", "/api/admin/users/2/admin", "grant=1"),
                ("POST", "/api/admin/users/2/delete", ""),
            ] {
                let resp = app
                    .clone()
                    .oneshot(req(method, uri, Some(cookie), body).await)
                    .await
                    .unwrap();
                assert_eq!(
                    resp.status(),
                    HttpStatus::NOT_FOUND,
                    "{method} {uri} must 404 for a non-owner"
                );
            }
        }

        // And none of it took effect: no new account, and the admin flags are
        // exactly as they were.
        let users = crate::db::list_users(&pool).await;
        assert_eq!(users.len(), 3, "a non-owner created an account");
        assert!(users.iter().all(|u| u.name != "mallory"));
        assert_eq!(
            users.iter().filter(|u| u.is_admin && !u.is_owner).count(),
            1,
            "admin flags were changed by a non-owner"
        );
    }

    /// Logged out is a redirect, not a 404 — the visitor genuinely needs to log
    /// in, and there is no signed-in session to protect the route's existence
    /// from.
    #[tokio::test]
    async fn test_user_management_redirects_when_logged_out() {
        let (app, _pool) = app_with_pool().await;
        let resp = app
            .clone()
            .oneshot(req("GET", "/admin/users", None, "").await)
            .await
            .unwrap();
        assert_eq!(resp.status(), HttpStatus::SEE_OTHER);
    }

    /// The owner can create an account, and the created account is a member.
    #[tokio::test]
    async fn test_owner_creates_a_member_account() {
        let (app, pool) = app_with_pool().await;
        let cookie = owner_cookie(&pool).await;

        let resp = app
            .clone()
            .oneshot(req("POST", "/admin/users", Some(&cookie), "name=alex&pin=1234").await)
            .await
            .unwrap();
        assert_eq!(resp.status(), HttpStatus::OK);

        let created = crate::db::get_user_by_name(&pool, "alex")
            .await
            .expect("account exists");
        assert!(
            !created.pin_hash.is_empty(),
            "the PIN was not hashed and stored"
        );
        assert_ne!(created.pin_hash, "1234", "the PIN was stored in plain text");
        assert!(crate::pin::verify_pin("1234", &created.pin_hash));
    }

    /// A too-short PIN is refused, and no half-made account is left behind.
    #[tokio::test]
    async fn test_create_rejects_a_short_pin() {
        let (app, pool) = app_with_pool().await;
        let cookie = owner_cookie(&pool).await;
        let resp = app
            .clone()
            .oneshot(req("POST", "/admin/users", Some(&cookie), "name=alex&pin=12").await)
            .await
            .unwrap();
        assert_eq!(resp.status(), HttpStatus::OK);
        assert!(crate::db::get_user_by_name(&pool, "alex").await.is_none());
    }

    /// The owner's own row renders with no destructive controls.
    #[tokio::test]
    async fn test_owner_row_has_no_controls() {
        let (app, pool) = app_with_pool().await;
        let cookie = owner_cookie(&pool).await;
        member_cookie(&pool, "alex", false).await;

        let resp = app
            .clone()
            .oneshot(req("GET", "/admin/users", Some(&cookie), "").await)
            .await
            .unwrap();
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let html = String::from_utf8(body.to_vec()).unwrap();

        // One delete form — for the member, not the owner (id 1).
        assert_eq!(html.matches("/delete").count(), 1);
        assert!(!html.contains("/api/admin/users/1/delete"));
        assert!(!html.contains("/api/admin/users/1/admin"));
    }
}
