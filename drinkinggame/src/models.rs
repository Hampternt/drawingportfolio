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

#[derive(sqlx::FromRow, Clone, Debug)]
pub struct RulePreset {
    pub id: i64,
    pub name: String,
    pub rules_json: String,
    pub created_at: String,
}

#[derive(sqlx::FromRow, Clone, Debug)]
pub struct Game {
    pub id: i64,
    pub room_id: i64,
    pub rules_json: String,
    pub deck_order: String,
    pub created_at: String,
    pub ended_at: Option<String>,
}

/// A draw joined with the drawer's name, for rendering.
#[derive(sqlx::FromRow, Clone, Debug)]
pub struct DrawRow {
    pub id: i64,
    pub player_id: i64,
    pub player_name: String,
    pub card_index: i64,
    pub spent_at: Option<String>,
}

#[derive(sqlx::FromRow, Clone, Debug, PartialEq)]
pub struct DrawCount {
    pub name: String,
    pub draws: i64,
}
