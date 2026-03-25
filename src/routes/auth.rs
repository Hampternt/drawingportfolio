use axum::{
    Router,
    routing::{get, post},
    extract::State,
    response::{Html, IntoResponse},
    http::{StatusCode, HeaderMap},
    Json,
};
use askama::Template;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;
use webauthn_rs::prelude::*;
use crate::{AppState, db, middleware};

#[derive(Template)]
#[template(path = "login.html")]
struct LoginTemplate;

#[derive(Template)]
#[template(path = "register.html")]
struct RegisterTemplate;

async fn login_page() -> impl IntoResponse {
    Html(LoginTemplate.render().unwrap())
}

async fn register_page(
    _: crate::middleware::LocalhostOnly,
) -> impl IntoResponse {
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
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR,
                   Json(serde_json::json!({ "ok": false, "error": e.to_string() }))).into_response(),
    }
}

async fn register_finish(
    _: crate::middleware::LocalhostOnly,
    State(state): State<Arc<AppState>>,
    Json(body): Json<FinishBody>,
) -> impl IntoResponse {
    let challenge = match db::take_challenge(&state.pool, &body.challenge_id).await {
        Some(c) => c,
        None => return Json(serde_json::json!({ "ok": false, "error": "challenge expired or invalid" })).into_response(),
    };

    let reg_state: PasskeyRegistration = match serde_json::from_str(&challenge.state_json) {
        Ok(s) => s,
        Err(_) => return Json(serde_json::json!({ "ok": false, "error": "invalid challenge state" })).into_response(),
    };

    let reg_response: RegisterPublicKeyCredential = match serde_json::from_value(body.credential) {
        Ok(r) => r,
        Err(e) => return Json(serde_json::json!({ "ok": false, "error": e.to_string() })).into_response(),
    };

    match state.webauthn.finish_passkey_registration(&reg_response, &reg_state) {
        Ok(passkey) => {
            let cred_id = serde_json::to_value(passkey.cred_id())
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_else(|| Uuid::new_v4().to_string());
            let passkey_json = serde_json::to_string(&passkey).unwrap();
            db::save_credential(&state.pool, &cred_id, &passkey_json).await;
            Json(serde_json::json!({ "ok": true })).into_response()
        }
        Err(e) => Json(serde_json::json!({ "ok": false, "error": e.to_string() })).into_response(),
    }
}

// Login

async fn login_start(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let passkeys: Vec<Passkey> = db::get_all_credentials(&state.pool)
        .await
        .iter()
        .filter_map(|c| serde_json::from_str(&c.passkey_json).ok())
        .collect();

    if passkeys.is_empty() {
        return (StatusCode::FORBIDDEN,
                Json(serde_json::json!({ "ok": false, "error": "no passkeys registered" }))).into_response();
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
            Json(serde_json::json!({ "challenge_id": challenge_id, "options": options })).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR,
                   Json(serde_json::json!({ "ok": false, "error": e.to_string() }))).into_response(),
    }
}

async fn login_finish(
    State(state): State<Arc<AppState>>,
    Json(body): Json<FinishBody>,
) -> impl IntoResponse {
    let challenge = match db::take_challenge(&state.pool, &body.challenge_id).await {
        Some(c) => c,
        None => return Json(serde_json::json!({ "ok": false, "error": "challenge expired" })).into_response(),
    };

    let auth_state: PasskeyAuthentication = match serde_json::from_str(&challenge.state_json) {
        Ok(s) => s,
        Err(_) => return Json(serde_json::json!({ "ok": false, "error": "invalid state" })).into_response(),
    };

    let auth_response: PublicKeyCredential = match serde_json::from_value(body.credential) {
        Ok(r) => r,
        Err(e) => return Json(serde_json::json!({ "ok": false, "error": e.to_string() })).into_response(),
    };

    match state.webauthn.finish_passkey_authentication(&auth_response, &auth_state) {
        Ok(_result) => {
            let session_id = Uuid::new_v4().to_string();
            let expires = chrono::Utc::now()
                .checked_add_signed(chrono::Duration::days(30))
                .unwrap()
                .format("%Y-%m-%dT%H:%M:%S")
                .to_string();
            db::create_session(&state.pool, &session_id, &expires).await;

            let cookie = middleware::make_session_cookie(&session_id);
            let mut headers = axum::http::HeaderMap::new();
            headers.insert(
                axum::http::header::SET_COOKIE,
                cookie.parse().unwrap(),
            );
            (headers, Json(serde_json::json!({ "ok": true }))).into_response()
        }
        Err(e) => Json(serde_json::json!({ "ok": false, "error": e.to_string() })).into_response(),
    }
}

async fn logout(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if let Some(cookies) = headers.get("cookie").and_then(|v| v.to_str().ok()) {
        for cookie in cookies.split(';') {
            if let Some(id) = cookie.trim().strip_prefix("session=") {
                db::delete_session(&state.pool, id).await;
            }
        }
    }
    let mut resp_headers = axum::http::HeaderMap::new();
    resp_headers.insert(
        axum::http::header::SET_COOKIE,
        "session=; HttpOnly; SameSite=Strict; Max-Age=0; Path=/".parse().unwrap(),
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
