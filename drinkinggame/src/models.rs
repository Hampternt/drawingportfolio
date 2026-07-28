//! Plain data structs mirroring database rows. FromRow lets sqlx's
//! runtime-checked query_as map columns by name.

#[derive(sqlx::FromRow, Clone, Debug)]
pub struct Player {
    pub id: i64,
    pub name: String,
    pub pin_hash: String,
    pub created_at: String,
}

#[derive(sqlx::FromRow, Clone, Debug)]
pub struct Room {
    pub id: i64,
    pub code: String,
    pub created_at: String,
    pub last_activity_at: String,
    pub ended_at: Option<String>,
}

#[derive(sqlx::FromRow, Clone, Debug, PartialEq)]
pub struct LeaderboardRow {
    pub name: String,
    pub drinks: i64,
    pub shots: i64,
}
