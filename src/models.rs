use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Post {
    pub id: i64,
    pub caption: String,
    pub image_url: String,
    pub format: String,
    pub file_size_bytes: i64,
    pub created_at: String,
}

/// Extensibility hook: add new variants here as post formats are implemented.
#[derive(Debug, Clone, PartialEq)]
pub enum PostFormat {
    Single,
}

impl PostFormat {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Single => "single",
        }
    }
}

impl Default for PostFormat {
    fn default() -> Self {
        Self::Single
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Session {
    pub id: String,
    pub expires_at: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PasskeyCredential {
    pub id: String,
    pub passkey_json: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AuthChallengeState {
    pub id: String,
    pub state_json: String,
}
