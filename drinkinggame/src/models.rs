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
    pub id: i64,
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
    pub kind: String,
    pub state_json: Option<String>,
}

/// A draw joined with the drawer's name, for rendering.
#[derive(sqlx::FromRow, Clone, Debug)]
pub struct DrawRow {
    pub id: i64,
    pub player_id: i64,
    pub player_name: String,
    pub card_index: i64,
    pub spent_at: Option<String>,
    pub rank: i64,
}

#[derive(sqlx::FromRow, Clone, Debug, PartialEq)]
pub struct DrawCount {
    pub name: String,
    pub draws: i64,
}

/// A house rule typed in after drawing a Jack. draw_id is UNIQUE at the DB
/// level — one rule per Jack, server-verifiable.
#[derive(sqlx::FromRow, Clone, Debug, PartialEq)]
pub struct HouseRule {
    pub id: i64,
    pub draw_id: i64,
    pub player_id: i64,
    pub player_name: String,
    pub text: String,
}

#[derive(sqlx::FromRow, Clone, Debug, PartialEq)]
pub struct RoomMember {
    pub id: i64,
    pub name: String,
    pub joined_at: String,
}
