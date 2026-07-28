//! Name + 4-digit-PIN identity. Argon2 hashing is CPU-bound (~tens of ms on
//! the cx23), so async callers run hash/verify inside spawn_blocking.

use argon2::password_hash::{
    rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString,
};
use argon2::Argon2;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::response::{IntoResponse, Redirect};
use rand::RngCore;

use crate::db::{self, DbPool};
use crate::error::GameError;
use crate::models::Player;
use crate::GameState;

pub const COOKIE_NAME: &str = "dg_session";

pub fn validate_name(name: &str) -> Result<String, GameError> {
    let trimmed = name.trim();
    if trimmed.is_empty() || trimmed.chars().count() > 20 {
        return Err(GameError::InvalidName);
    }
    Ok(trimmed.to_string())
}

pub fn validate_pin(pin: &str) -> Result<(), GameError> {
    if pin.len() == 4 && pin.bytes().all(|b| b.is_ascii_digit()) {
        Ok(())
    } else {
        Err(GameError::InvalidPin)
    }
}

pub fn hash_pin(pin: &str) -> String {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(pin.as_bytes(), &salt)
        .expect("argon2 hashing failed")
        .to_string()
}

pub fn verify_pin(pin: &str, hash: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(hash) else {
        return false;
    };
    Argon2::default()
        .verify_password(pin.as_bytes(), &parsed)
        .is_ok()
}

/// Known name -> verify PIN. Unknown name -> register. A lost create race
/// (two phones registering the same name simultaneously) falls back to verify.
pub async fn login_or_register(pool: &DbPool, name: &str, pin: &str) -> Result<Player, GameError> {
    let name = validate_name(name)?;
    validate_pin(pin)?;

    if let Some(player) = db::get_player_by_name(pool, &name).await {
        return check_pin(player, pin).await;
    }

    let pin_owned = pin.to_string();
    let hash = tokio::task::spawn_blocking(move || hash_pin(&pin_owned))
        .await
        .expect("hash task panicked");

    match db::insert_player(pool, &name, &hash).await {
        Ok(_) => Ok(db::get_player_by_name(pool, &name)
            .await
            .expect("player row must exist after insert")),
        // UNIQUE violation: someone else registered this name between our
        // lookup and insert. Treat it as a login attempt against their PIN.
        Err(_) => {
            let player = db::get_player_by_name(pool, &name)
                .await
                .ok_or(GameError::WrongPin)?;
            check_pin(player, pin).await
        }
    }
}

async fn check_pin(player: Player, pin: &str) -> Result<Player, GameError> {
    let pin = pin.to_string();
    let hash = player.pin_hash.clone();
    let ok = tokio::task::spawn_blocking(move || verify_pin(&pin, &hash))
        .await
        .unwrap_or(false);
    if ok {
        Ok(player)
    } else {
        Err(GameError::WrongPin)
    }
}

/// 32 random bytes as 64 hex chars — same entropy class as the portfolio's
/// session IDs.
pub fn new_session_id() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// SameSite=Lax (not Strict): the join flow follows links shared between
/// phones, and Strict would drop the cookie on those navigations.
pub fn session_cookie(id: &str) -> String {
    format!("{COOKIE_NAME}={id}; HttpOnly; SameSite=Lax; Max-Age=7776000; Path=/")
}

fn extract_session_cookie(parts: &Parts) -> Option<String> {
    let cookies = parts.headers.get("cookie")?.to_str().ok()?;
    for cookie in cookies.split(';') {
        if let Some(val) = cookie.trim().strip_prefix(&format!("{COOKIE_NAME}=")) {
            return Some(val.to_string());
        }
    }
    None
}

/// Requires a live session; otherwise redirects to the game landing page.
pub struct PlayerSession(pub Player);

impl FromRequestParts<GameState> for PlayerSession {
    type Rejection = axum::response::Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &GameState,
    ) -> Result<Self, Self::Rejection> {
        if let Some(id) = extract_session_cookie(parts) {
            if let Some(player) = db::get_session_player(&state.pool, &id).await {
                return Ok(PlayerSession(player));
            }
        }
        Err(Redirect::to(&format!("{}/", state.base_path)).into_response())
    }
}

/// Checks the session without ever rejecting.
pub struct OptionalPlayer(pub Option<Player>);

impl FromRequestParts<GameState> for OptionalPlayer {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &GameState,
    ) -> Result<Self, Self::Rejection> {
        let player = match extract_session_cookie(parts) {
            Some(id) => db::get_session_player(&state.pool, &id).await,
            None => None,
        };
        Ok(OptionalPlayer(player))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pin_hash_roundtrip() {
        let hash = hash_pin("1234");
        assert!(verify_pin("1234", &hash));
        assert!(!verify_pin("4321", &hash));
        assert!(!verify_pin("1234", "not-a-hash"));
    }

    #[test]
    fn test_validation() {
        assert!(validate_pin("1234").is_ok());
        assert!(validate_pin("123").is_err());
        assert!(validate_pin("12345").is_err());
        assert!(validate_pin("12a4").is_err());
        assert_eq!(validate_name("  bob  ").unwrap(), "bob");
        assert!(validate_name("   ").is_err());
        assert!(validate_name(&"x".repeat(21)).is_err());
    }

    #[test]
    fn test_session_id_shape() {
        let a = new_session_id();
        let b = new_session_id();
        assert_eq!(a.len(), 64);
        assert_ne!(a, b);
        assert!(a.bytes().all(|c| c.is_ascii_hexdigit()));
    }

    #[tokio::test]
    async fn test_login_or_register_flow() {
        let pool = crate::db::test_pool().await;
        // First contact registers.
        let p1 = login_or_register(&pool, "alice", "1234").await.unwrap();
        // Same name + right PIN logs in as the same player.
        let p2 = login_or_register(&pool, "alice", "1234").await.unwrap();
        assert_eq!(p1.id, p2.id);
        // Wrong PIN is rejected.
        assert!(matches!(
            login_or_register(&pool, "alice", "9999").await,
            Err(GameError::WrongPin)
        ));
        // Bad inputs are rejected before touching the db.
        assert!(matches!(
            login_or_register(&pool, "", "1234").await,
            Err(GameError::InvalidName)
        ));
        assert!(matches!(
            login_or_register(&pool, "bob", "12").await,
            Err(GameError::InvalidPin)
        ));
    }
}
