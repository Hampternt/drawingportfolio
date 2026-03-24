use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Post {
    pub id: i64,
    pub caption: String,
    pub image_url: String,
    pub created_at: String,
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
