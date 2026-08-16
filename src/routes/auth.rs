use crate::{db, middleware, AppState};
use askama::Template;
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;
use webauthn_rs::prelude::*;

#[derive(Template)]
#[template(path = "login.html")]
struct LoginTemplate;

#[derive(Template)]
#[template(path = "register.html")]
struct RegisterTemplate;

async fn login_page() -> impl IntoResponse {
    Html(LoginTemplate.render().unwrap())
}

async fn register_page(_: crate::middleware::LocalhostOnly) -> impl IntoResponse {
    Html(RegisterTemplate.render().unwrap())
}

#[derive(Serialize)]
struct StartResponse {
    challenge_id: String,
    options: serde_json::Value,
}

#[derive(Deserialize)]
struct FinishBody {
    challenge_id: String,
    credential: serde_json::Value,
}

/// The storage key for a credential id, derived identically at registration and
/// at login.
///
/// `CredentialID` serialises to a base64url JSON string. Registration stores
/// that string as `passkey_credentials.id`; login re-derives it from the
/// ceremony result to find out *who* just authenticated. The two must agree
/// exactly — hence one function rather than two call sites repeating the same
/// three steps, and a `None` on failure rather than the random UUID this used
/// to substitute, which would have stored a credential under a key login could
/// never derive.
fn cred_id_key(cred_id: &CredentialID) -> Option<String> {
    serde_json::to_value(cred_id)
        .ok()?
        .as_str()
        .map(str::to_owned)
}

// Registration (localhost only)

async fn register_start(
    _: crate::middleware::LocalhostOnly,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let user_id = Uuid::new_v4();

    let existing: Vec<CredentialID> = db::get_all_credentials(&state.pool)
        .await
        .iter()
        .filter_map(|c| serde_json::from_str::<Passkey>(&c.passkey_json).ok())
        .map(|pk| pk.cred_id().clone())
        .collect();

    match state.webauthn.start_passkey_registration(
        user_id,
        "admin",
        "Portfolio Admin",
        Some(existing),
    ) {
        Ok((ccr, reg_state)) => {
            let challenge_id = Uuid::new_v4().to_string();
            let state_json = serde_json::to_string(&reg_state).unwrap();
            let expires = chrono::Utc::now()
                .checked_add_signed(chrono::Duration::minutes(5))
                .unwrap()
                .format("%Y-%m-%dT%H:%M:%S")
                .to_string();
            db::save_challenge(&state.pool, &challenge_id, &state_json, &expires).await;
            let options = serde_json::to_value(&ccr).unwrap();
            Json(serde_json::json!({ "challenge_id": challenge_id, "options": options }))
                .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "ok": false, "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn register_finish(
    _: crate::middleware::LocalhostOnly,
    State(state): State<Arc<AppState>>,
    Json(body): Json<FinishBody>,
) -> impl IntoResponse {
    let challenge = match db::take_challenge(&state.pool, &body.challenge_id).await {
        Some(c) => c,
        None => {
            return Json(
                serde_json::json!({ "ok": false, "error": "challenge expired or invalid" }),
            )
            .into_response()
        }
    };

    let reg_state: PasskeyRegistration = match serde_json::from_str(&challenge.state_json) {
        Ok(s) => s,
        Err(_) => {
            return Json(serde_json::json!({ "ok": false, "error": "invalid challenge state" }))
                .into_response()
        }
    };

    let reg_response: RegisterPublicKeyCredential = match serde_json::from_value(body.credential) {
        Ok(r) => r,
        Err(e) => {
            return Json(serde_json::json!({ "ok": false, "error": e.to_string() })).into_response()
        }
    };

    match state
        .webauthn
        .finish_passkey_registration(&reg_response, &reg_state)
    {
        Ok(passkey) => {
            let cred_id = match cred_id_key(passkey.cred_id()) {
                Some(k) => k,
                None => {
                    tracing::error!("could not derive credential key; refusing to store passkey");
                    return Json(
                        serde_json::json!({ "ok": false, "error": "credential id not serialisable" }),
                    )
                    .into_response();
                }
            };
            // Registration is localhost-only, so the person at the keyboard is
            // the owner. Members get their passkey through the management page
            // in pack 3, which supplies its own target user.
            let owner_id = match db::get_owner_user_id(&state.pool).await {
                Some(id) => id,
                None => {
                    tracing::error!("no owner row — migration 015 did not run");
                    return Json(
                        serde_json::json!({ "ok": false, "error": "no owner configured" }),
                    )
                    .into_response();
                }
            };
            let passkey_json = serde_json::to_string(&passkey).unwrap();
            db::save_credential(&state.pool, &cred_id, &passkey_json, owner_id).await;
            tracing::info!("passkey registered successfully, cred_id={cred_id}");
            Json(serde_json::json!({ "ok": true })).into_response()
        }
        Err(e) => Json(serde_json::json!({ "ok": false, "error": e.to_string() })).into_response(),
    }
}

// Login

async fn login_start(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let passkeys: Vec<Passkey> = db::get_all_credentials(&state.pool)
        .await
        .iter()
        .filter_map(|c| serde_json::from_str(&c.passkey_json).ok())
        .collect();

    if passkeys.is_empty() {
        tracing::warn!("login attempted but no passkeys registered");
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({ "ok": false, "error": "no passkeys registered" })),
        )
            .into_response();
    }

    match state.webauthn.start_passkey_authentication(&passkeys) {
        Ok((rcr, auth_state)) => {
            let challenge_id = Uuid::new_v4().to_string();
            let state_json = serde_json::to_string(&auth_state).unwrap();
            let expires = chrono::Utc::now()
                .checked_add_signed(chrono::Duration::minutes(5))
                .unwrap()
                .format("%Y-%m-%dT%H:%M:%S")
                .to_string();
            db::save_challenge(&state.pool, &challenge_id, &state_json, &expires).await;
            let options = serde_json::to_value(&rcr).unwrap();
            Json(serde_json::json!({ "challenge_id": challenge_id, "options": options }))
                .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "ok": false, "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn login_finish(
    State(state): State<Arc<AppState>>,
    Json(body): Json<FinishBody>,
) -> impl IntoResponse {
    let challenge = match db::take_challenge(&state.pool, &body.challenge_id).await {
        Some(c) => c,
        None => {
            return Json(serde_json::json!({ "ok": false, "error": "challenge expired" }))
                .into_response()
        }
    };

    let auth_state: PasskeyAuthentication = match serde_json::from_str(&challenge.state_json) {
        Ok(s) => s,
        Err(_) => {
            return Json(serde_json::json!({ "ok": false, "error": "invalid state" }))
                .into_response()
        }
    };

    let auth_response: PublicKeyCredential = match serde_json::from_value(body.credential) {
        Ok(r) => r,
        Err(e) => {
            return Json(serde_json::json!({ "ok": false, "error": e.to_string() })).into_response()
        }
    };

    match state
        .webauthn
        .finish_passkey_authentication(&auth_response, &auth_state)
    {
        Ok(result) => {
            // Which passkey answered the challenge decides *who* is logged in.
            // `login_start` offers every registered credential, so without this
            // lookup a session could only ever mean "somebody valid".
            let cred_id = match cred_id_key(result.cred_id()) {
                Some(k) => k,
                None => {
                    tracing::warn!("login: could not derive credential key");
                    return Json(serde_json::json!({ "ok": false, "error": "unknown credential" }))
                        .into_response();
                }
            };
            let user_id = match db::get_credential_user_id(&state.pool, &cred_id).await {
                Some(id) => id,
                None => {
                    // The ceremony passed but the credential has no user. Fail
                    // closed rather than defaulting to the owner.
                    tracing::warn!("login: credential {cred_id} is not bound to a user");
                    return Json(serde_json::json!({ "ok": false, "error": "unknown credential" }))
                        .into_response();
                }
            };

            let session_id = Uuid::new_v4().to_string();
            let expires = chrono::Utc::now()
                .checked_add_signed(chrono::Duration::days(30))
                .unwrap()
                .format("%Y-%m-%dT%H:%M:%S")
                .to_string();
            db::create_session(&state.pool, &session_id, &expires, user_id).await;
            tracing::info!("login successful, session created for user {user_id}");

            let cookie = middleware::make_session_cookie(&session_id);
            let mut headers = axum::http::HeaderMap::new();
            headers.insert(axum::http::header::SET_COOKIE, cookie.parse().unwrap());
            (headers, Json(serde_json::json!({ "ok": true }))).into_response()
        }
        Err(e) => {
            tracing::warn!("login failed: {e}");
            Json(serde_json::json!({ "ok": false, "error": e.to_string() })).into_response()
        }
    }
}

async fn logout(State(state): State<Arc<AppState>>, headers: HeaderMap) -> impl IntoResponse {
    if let Some(cookies) = headers.get("cookie").and_then(|v| v.to_str().ok()) {
        for cookie in cookies.split(';') {
            if let Some(id) = cookie.trim().strip_prefix("session=") {
                tracing::info!("logout: deleting session");
                db::delete_session(&state.pool, id).await;
            }
        }
    }
    let mut resp_headers = axum::http::HeaderMap::new();
    resp_headers.insert(
        axum::http::header::SET_COOKIE,
        "session=; HttpOnly; SameSite=Strict; Max-Age=0; Path=/"
            .parse()
            .unwrap(),
    );
    resp_headers.insert(
        axum::http::header::LOCATION,
        "/admin/login".parse().unwrap(),
    );
    (StatusCode::SEE_OTHER, resp_headers).into_response()
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/admin/login", get(login_page))
        .route("/admin/register", get(register_page))
        .route("/api/auth/login/start", post(login_start))
        .route("/api/auth/login/finish", post(login_finish))
        .route("/api/auth/logout", post(logout))
        .route("/api/auth/register/start", post(register_start))
        .route("/api/auth/register/finish", post(register_finish))
}
