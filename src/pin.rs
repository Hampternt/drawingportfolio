//! Name + PIN credentials for fitness accounts.
//!
//! The portfolio's own login is a passkey ceremony, which is strictly better
//! but only works on a device that holds the credential. `/fitness` is shared
//! with people who just need to log their lunch from their own phone, so it
//! also accepts a name and a PIN — the owner's decision, on the grounds that
//! this part of the site is not security-critical.
//!
//! "Not critical" is not "unprotected". A PIN is a small secret over a public
//! endpoint, so two things carry the weight: Argon2 makes each guess expensive,
//! and [`crate::db::record_failed_pin`] locks the account after a handful of
//! wrong ones. Without the lockout, a 4-digit PIN is 10,000 guesses and Argon2
//! only decides how long that takes.
//!
//! The hashing follows `drinkinggame/src/auth.rs`, which has been in
//! production here since 2026-08: Argon2 is CPU-bound at tens of milliseconds,
//! so every hash and verify runs inside `spawn_blocking` rather than stalling
//! the async runtime.

use argon2::password_hash::{
    rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString,
};
use argon2::Argon2;

/// Wrong PINs before the account locks.
pub const MAX_PIN_ATTEMPTS: i64 = 5;

/// How long a locked account stays locked.
pub const LOCKOUT_MINUTES: i64 = 15;

/// Shortest acceptable PIN. Four digits matches the drinking game and is what
/// people actually type on a phone.
pub const MIN_PIN_LEN: usize = 4;
pub const MAX_PIN_LEN: usize = 12;

#[derive(Debug, PartialEq, Eq)]
pub enum CredentialError {
    InvalidName,
    NameTaken,
    InvalidPin,
    WrongPin,
    Locked,
}

impl CredentialError {
    /// The message shown to whoever is trying to log in.
    ///
    /// `WrongPin` deliberately does not distinguish "no such account" from
    /// "wrong PIN" — telling them apart hands an attacker a list of valid
    /// names, and the honest version helps nobody who has actually forgotten
    /// which name they used.
    pub fn message(&self) -> &'static str {
        match self {
            CredentialError::InvalidName => "Name must be 1–20 characters",
            CredentialError::NameTaken => "That name is already taken",
            CredentialError::InvalidPin => "PIN must be 4–12 digits",
            CredentialError::WrongPin => "Wrong name or PIN",
            CredentialError::Locked => {
                "Too many wrong PINs — this account is locked for 15 minutes"
            }
        }
    }
}

pub fn validate_name(name: &str) -> Result<String, CredentialError> {
    let trimmed = name.trim();
    if trimmed.is_empty() || trimmed.chars().count() > 20 {
        return Err(CredentialError::InvalidName);
    }
    Ok(trimmed.to_string())
}

pub fn validate_pin(pin: &str) -> Result<(), CredentialError> {
    let len = pin.len();
    if (MIN_PIN_LEN..=MAX_PIN_LEN).contains(&len) && pin.bytes().all(|b| b.is_ascii_digit()) {
        Ok(())
    } else {
        Err(CredentialError::InvalidPin)
    }
}

/// Argon2 hash of a PIN. **CPU-bound** — call via [`hash_pin_async`] from a
/// request handler.
pub fn hash_pin(pin: &str) -> String {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(pin.as_bytes(), &salt)
        .expect("argon2 hashing failed")
        .to_string()
}

/// Constant-time-ish verify. A malformed or absent hash is a failure, never a
/// pass — an account with no PIN set (the owner, who uses a passkey) must not
/// be loggable by supplying an empty one.
pub fn verify_pin(pin: &str, hash: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(hash) else {
        return false;
    };
    Argon2::default()
        .verify_password(pin.as_bytes(), &parsed)
        .is_ok()
}

/// Hashes off the async runtime. Argon2 takes tens of milliseconds; doing it
/// inline blocks every other request sharing the worker thread.
pub async fn hash_pin_async(pin: &str) -> String {
    let owned = pin.to_string();
    tokio::task::spawn_blocking(move || hash_pin(&owned))
        .await
        .expect("argon2 hash task panicked")
}

/// Verifies off the async runtime. A panic in the blocking task reads as a
/// failed login rather than propagating.
pub async fn verify_pin_async(pin: &str, hash: &str) -> bool {
    let pin = pin.to_string();
    let hash = hash.to_string();
    tokio::task::spawn_blocking(move || verify_pin(&pin, &hash))
        .await
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pin_hash_roundtrip() {
        let hash = hash_pin("1234");
        assert!(verify_pin("1234", &hash));
        assert!(!verify_pin("4321", &hash));
    }

    /// An account with no usable hash must never verify.
    ///
    /// The owner authenticates with a passkey and may have `pin_hash` NULL
    /// forever; `get_user_by_name` hands that through as an empty string. If a
    /// malformed hash returned `true`, an empty PIN would log in as the owner.
    #[test]
    fn test_absent_or_malformed_hash_never_verifies() {
        assert!(!verify_pin("1234", ""));
        assert!(!verify_pin("", ""));
        assert!(!verify_pin("1234", "not-a-hash"));
        assert!(!verify_pin("", "not-a-hash"));
    }

    #[test]
    fn test_validate_pin() {
        assert!(validate_pin("1234").is_ok());
        assert!(validate_pin("123456789012").is_ok());
        assert!(validate_pin("123").is_err(), "too short");
        assert!(validate_pin("1234567890123").is_err(), "too long");
        assert!(validate_pin("12a4").is_err(), "not all digits");
        assert!(validate_pin("").is_err());
        assert!(
            validate_pin(" 123").is_err(),
            "no trimming — a space is not a digit"
        );
    }

    #[test]
    fn test_validate_name() {
        assert_eq!(validate_name("  bob  ").unwrap(), "bob");
        assert!(validate_name("   ").is_err());
        assert!(validate_name(&"x".repeat(21)).is_err());
        assert!(validate_name(&"x".repeat(20)).is_ok());
    }

    /// Salted: the same PIN hashes differently every time, so equal hashes in
    /// the table never reveal equal PINs.
    #[test]
    fn test_hashes_are_salted() {
        assert_ne!(hash_pin("1234"), hash_pin("1234"));
    }

    /// A wrong name and a wrong PIN are indistinguishable to the caller.
    #[test]
    fn test_wrong_pin_message_does_not_leak_account_existence() {
        assert_eq!(CredentialError::WrongPin.message(), "Wrong name or PIN");
    }
}
