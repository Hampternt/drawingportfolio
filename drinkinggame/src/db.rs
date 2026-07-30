use crate::error::GameError;
use crate::models::{
    DrawCount, DrawRow, Game, HouseRule, LeaderboardRow, Player, Room, RoomMember, RulePreset,
};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::str::FromStr;

pub type DbPool = sqlx::SqlitePool;

pub async fn connect(url: &str) -> DbPool {
    let opts = SqliteConnectOptions::from_str(url)
        .expect("invalid drinks DATABASE_URL")
        .create_if_missing(true);
    SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(opts)
        .await
        .expect("failed to connect to drinks db")
}

pub async fn run_migrations(pool: &DbPool) {
    sqlx::query(include_str!("../migrations/001_initial.sql"))
        .execute(pool)
        .await
        .expect("drinks migration 001 failed");
    sqlx::query(include_str!("../migrations/002_ring_of_fire.sql"))
        .execute(pool)
        .await
        .expect("drinks migration 002 failed");
    sqlx::query(include_str!("../migrations/003_shell_and_three_man.sql"))
        .execute(pool)
        .await
        .expect("drinks migration 003 failed");
    // Seed guard: recreate the Standard preset only if missing, so deleting
    // it is permitted but it returns on next deploy (accepted v1 quirk).
    sqlx::query("INSERT OR IGNORE INTO rule_presets (name, rules_json) VALUES ('Standard', ?1)")
        .bind(crate::rules::standard_rules_json())
        .execute(pool)
        .await
        .expect("standard preset seed failed");

    // 003 ALTERs — not idempotent in SQLite, so guard each with pragma_table_info.
    if !column_exists(pool, "games", "kind").await {
        sqlx::query("ALTER TABLE games ADD COLUMN kind TEXT NOT NULL DEFAULT 'ring_of_fire'")
            .execute(pool)
            .await
            .expect("003 kind");
    }
    if !column_exists(pool, "games", "state_json").await {
        sqlx::query("ALTER TABLE games ADD COLUMN state_json TEXT")
            .execute(pool)
            .await
            .expect("003 state_json");
    }
    if !column_exists(pool, "game_draws", "rank").await {
        sqlx::query("ALTER TABLE game_draws ADD COLUMN rank INTEGER")
            .execute(pool)
            .await
            .expect("003 rank");
    }
    // Rank backfill — WHERE rank IS NULL makes it idempotent.
    let games: Vec<(i64, String)> = sqlx::query_as(
        "SELECT id, deck_order FROM games WHERE deck_order != '' AND id IN
         (SELECT DISTINCT game_id FROM game_draws WHERE rank IS NULL)",
    )
    .fetch_all(pool)
    .await
    .expect("backfill scan");
    for (gid, deck_order) in games {
        let deck = crate::cards::parse_deck(&deck_order);
        let draws: Vec<(i64, i64)> = sqlx::query_as(
            "SELECT id, card_index FROM game_draws WHERE game_id = ?1 AND rank IS NULL",
        )
        .bind(gid)
        .fetch_all(pool)
        .await
        .expect("backfill read");
        for (id, idx) in draws {
            sqlx::query("UPDATE game_draws SET rank = ?1 WHERE id = ?2")
                .bind(deck[idx as usize].rank as i64)
                .bind(id)
                .execute(pool)
                .await
                .expect("backfill write");
        }
    }
}

async fn column_exists(pool: &DbPool, table: &str, column: &str) -> bool {
    let cols: Vec<(String,)> =
        sqlx::query_as(&format!("SELECT name FROM pragma_table_info('{table}')"))
            .fetch_all(pool)
            .await
            .expect("pragma_table_info failed");
    cols.iter().any(|(c,)| c == column)
}

pub async fn get_player_by_name(pool: &DbPool, name: &str) -> Option<Player> {
    sqlx::query_as::<_, Player>(
        "SELECT id, name, pin_hash, created_at FROM players WHERE name = ?1",
    )
    .bind(name)
    .fetch_optional(pool)
    .await
    .expect("get_player_by_name failed")
}

/// Returns Err on UNIQUE violation (name taken) — callers handle the race.
pub async fn insert_player(pool: &DbPool, name: &str, pin_hash: &str) -> Result<i64, sqlx::Error> {
    let res = sqlx::query("INSERT INTO players (name, pin_hash) VALUES (?1, ?2)")
        .bind(name)
        .bind(pin_hash)
        .execute(pool)
        .await?;
    Ok(res.last_insert_rowid())
}

/// ttl is a SQLite datetime modifier, e.g. "+90 days". Tests pass "-1 days"
/// to create an already-expired session.
pub async fn create_session(pool: &DbPool, id: &str, player_id: i64, ttl: &str) {
    sqlx::query(
        "INSERT INTO sessions (id, player_id, expires_at) VALUES (?1, ?2, datetime('now', ?3))",
    )
    .bind(id)
    .bind(player_id)
    .bind(ttl)
    .execute(pool)
    .await
    .expect("create_session failed");
}

pub async fn get_session_player(pool: &DbPool, session_id: &str) -> Option<Player> {
    sqlx::query_as::<_, Player>(
        "SELECT p.id, p.name, p.pin_hash, p.created_at
         FROM sessions s JOIN players p ON p.id = s.player_id
         WHERE s.id = ?1 AND s.expires_at > datetime('now')",
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await
    .expect("get_session_player failed")
}

pub async fn cleanup_expired_sessions(pool: &DbPool) {
    sqlx::query("DELETE FROM sessions WHERE expires_at <= datetime('now')")
        .execute(pool)
        .await
        .expect("cleanup_expired_sessions failed");
}

pub async fn insert_room(pool: &DbPool, code: &str) -> Result<i64, sqlx::Error> {
    let res = sqlx::query("INSERT INTO rooms (code) VALUES (?1)")
        .bind(code)
        .execute(pool)
        .await?;
    Ok(res.last_insert_rowid())
}

pub async fn get_open_room(pool: &DbPool, code: &str) -> Option<Room> {
    sqlx::query_as::<_, Room>(
        "SELECT id, code, created_at, last_activity_at, ended_at
         FROM rooms WHERE code = ?1 AND ended_at IS NULL",
    )
    .bind(code)
    .fetch_optional(pool)
    .await
    .expect("get_open_room failed")
}

pub async fn get_room_by_id(pool: &DbPool, room_id: i64) -> Option<Room> {
    sqlx::query_as::<_, Room>(
        "SELECT id, code, created_at, last_activity_at, ended_at
         FROM rooms WHERE id = ?1",
    )
    .bind(room_id)
    .fetch_optional(pool)
    .await
    .expect("get_room_by_id failed")
}

/// Idempotent: rejoining is a no-op apart from the activity bump.
pub async fn join_room(pool: &DbPool, room_id: i64, player_id: i64) {
    sqlx::query("INSERT OR IGNORE INTO room_players (room_id, player_id) VALUES (?1, ?2)")
        .bind(room_id)
        .bind(player_id)
        .execute(pool)
        .await
        .expect("join_room failed");
    touch_room(pool, room_id).await;
}

pub async fn is_room_member(pool: &DbPool, room_id: i64, player_id: i64) -> bool {
    let row: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM room_players WHERE room_id = ?1 AND player_id = ?2")
            .bind(room_id)
            .bind(player_id)
            .fetch_one(pool)
            .await
            .expect("is_room_member failed");
    row.0 > 0
}

pub async fn touch_room(pool: &DbPool, room_id: i64) {
    sqlx::query("UPDATE rooms SET last_activity_at = datetime('now') WHERE id = ?1")
        .bind(room_id)
        .execute(pool)
        .await
        .expect("touch_room failed");
}

pub async fn end_room(pool: &DbPool, room_id: i64) {
    sqlx::query("UPDATE rooms SET ended_at = datetime('now') WHERE id = ?1 AND ended_at IS NULL")
        .bind(room_id)
        .execute(pool)
        .await
        .expect("end_room failed");
    sqlx::query(
        "UPDATE games SET ended_at = datetime('now') WHERE room_id = ?1 AND ended_at IS NULL",
    )
    .bind(room_id)
    .execute(pool)
    .await
    .expect("end_room: end active game failed");
}

/// Ends rooms idle longer than max_idle_hours; returns their ids so the
/// caller can drop broadcast channels.
pub async fn end_inactive_rooms(pool: &DbPool, max_idle_hours: i64) -> Vec<i64> {
    let modifier = format!("-{max_idle_hours} hours");
    let ids: Vec<(i64,)> = sqlx::query_as(
        "UPDATE rooms SET ended_at = datetime('now')
         WHERE ended_at IS NULL AND last_activity_at < datetime('now', ?1)
         RETURNING id",
    )
    .bind(&modifier)
    .fetch_all(pool)
    .await
    .expect("end_inactive_rooms failed");
    let ids: Vec<i64> = ids.into_iter().map(|(id,)| id).collect();
    for room_id in &ids {
        sqlx::query(
            "UPDATE games SET ended_at = datetime('now') WHERE room_id = ?1 AND ended_at IS NULL",
        )
        .bind(room_id)
        .execute(pool)
        .await
        .expect("end_inactive_rooms: end active game failed");
    }
    ids
}

pub async fn insert_event(pool: &DbPool, room_id: i64, player_id: i64, kind: &str) {
    sqlx::query("INSERT INTO events (room_id, player_id, kind) VALUES (?1, ?2, ?3)")
        .bind(room_id)
        .bind(player_id)
        .bind(kind)
        .execute(pool)
        .await
        .expect("insert_event failed");
}

/// Tombstones the caller's most recent live event in this room.
/// Returns false when there is nothing left to undo.
pub async fn undo_last_event(pool: &DbPool, room_id: i64, player_id: i64) -> bool {
    let res = sqlx::query(
        "UPDATE events SET undone_at = datetime('now')
         WHERE id = (
             SELECT id FROM events
             WHERE room_id = ?1 AND player_id = ?2 AND undone_at IS NULL
             ORDER BY id DESC LIMIT 1
         )",
    )
    .bind(room_id)
    .bind(player_id)
    .execute(pool)
    .await
    .expect("undo_last_event failed");
    res.rows_affected() > 0
}

/// Per-room standings: every member appears (LEFT JOIN), zero rows and all.
/// Sorted by total descending, then name for a stable order.
pub async fn leaderboard(pool: &DbPool, room_id: i64) -> Vec<LeaderboardRow> {
    sqlx::query_as::<_, LeaderboardRow>(
        "SELECT p.id AS id, p.name,
                COALESCE(SUM(CASE WHEN e.kind = 'drink' THEN 1 ELSE 0 END), 0) AS drinks,
                COALESCE(SUM(CASE WHEN e.kind = 'shot'  THEN 1 ELSE 0 END), 0) AS shots
         FROM room_players rp
         JOIN players p ON p.id = rp.player_id
         LEFT JOIN events e
              ON e.room_id = rp.room_id
             AND e.player_id = rp.player_id
             AND e.undone_at IS NULL
         WHERE rp.room_id = ?1
         GROUP BY p.id
         ORDER BY (drinks + shots) DESC, p.name ASC",
    )
    .bind(room_id)
    .fetch_all(pool)
    .await
    .expect("leaderboard failed")
}

/// Members of a room in join order — used by games that need seat order
/// (Shell direction, 3 Man turn rotation).
pub async fn room_members(pool: &DbPool, room_id: i64) -> Vec<RoomMember> {
    sqlx::query_as::<_, RoomMember>(
        "SELECT p.id AS id, p.name, rp.joined_at
         FROM room_players rp JOIN players p ON p.id = rp.player_id
         WHERE rp.room_id = ?1
         ORDER BY rp.joined_at, p.id",
    )
    .bind(room_id)
    .fetch_all(pool)
    .await
    .expect("room_members failed")
}

/// Bulk-inserts n identical events at once — used by games that hand out
/// several drinks/shots in one action (e.g. a 3 Man penalty).
pub async fn insert_events_bulk(pool: &DbPool, room_id: i64, player_id: i64, kind: &str, n: u32) {
    for _ in 0..n {
        insert_event(pool, room_id, player_id, kind).await;
    }
}

pub async fn list_presets(pool: &DbPool) -> Vec<RulePreset> {
    sqlx::query_as::<_, RulePreset>(
        "SELECT id, name, rules_json, created_at FROM rule_presets ORDER BY id",
    )
    .fetch_all(pool)
    .await
    .expect("list_presets failed")
}

pub async fn get_preset(pool: &DbPool, id: i64) -> Option<RulePreset> {
    sqlx::query_as::<_, RulePreset>(
        "SELECT id, name, rules_json, created_at FROM rule_presets WHERE id = ?1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .expect("get_preset failed")
}

/// Returns Err on UNIQUE violation (name taken) — callers map it to a
/// friendly error.
pub async fn insert_preset(
    pool: &DbPool,
    name: &str,
    rules_json: &str,
) -> Result<i64, sqlx::Error> {
    let res = sqlx::query("INSERT INTO rule_presets (name, rules_json) VALUES (?1, ?2)")
        .bind(name)
        .bind(rules_json)
        .execute(pool)
        .await?;
    Ok(res.last_insert_rowid())
}

/// Ok(false) when the id doesn't exist; Err on a name collision.
pub async fn update_preset(
    pool: &DbPool,
    id: i64,
    name: &str,
    rules_json: &str,
) -> Result<bool, sqlx::Error> {
    let res = sqlx::query("UPDATE rule_presets SET name = ?2, rules_json = ?3 WHERE id = ?1")
        .bind(id)
        .bind(name)
        .bind(rules_json)
        .execute(pool)
        .await?;
    Ok(res.rows_affected() > 0)
}

pub async fn delete_preset(pool: &DbPool, id: i64) -> bool {
    let res = sqlx::query("DELETE FROM rule_presets WHERE id = ?1")
        .bind(id)
        .execute(pool)
        .await
        .expect("delete_preset failed");
    res.rows_affected() > 0
}

/// GameAlreadyActive when the partial unique index (one active game per
/// room) rejects the insert. `kind` distinguishes which game is running
/// ("ring_of_fire", "shell", "three_man"); `state_json` is the initial
/// per-game state blob for games that need one (None for Ring of Fire).
pub async fn start_game(
    pool: &DbPool,
    room_id: i64,
    kind: &str,
    rules_json: &str,
    deck_order: &str,
    state_json: Option<&str>,
) -> Result<i64, GameError> {
    let res = sqlx::query(
        "INSERT INTO games (room_id, kind, rules_json, deck_order, state_json)
         VALUES (?1, ?2, ?3, ?4, ?5)",
    )
    .bind(room_id)
    .bind(kind)
    .bind(rules_json)
    .bind(deck_order)
    .bind(state_json)
    .execute(pool)
    .await;
    match res {
        Ok(r) => Ok(r.last_insert_rowid()),
        Err(e)
            if e.as_database_error()
                .is_some_and(|d| d.is_unique_violation()) =>
        {
            Err(GameError::GameAlreadyActive)
        }
        Err(e) => Err(e.into()),
    }
}

pub async fn get_active_game(pool: &DbPool, room_id: i64) -> Option<Game> {
    sqlx::query_as::<_, Game>(
        "SELECT id, room_id, rules_json, deck_order, created_at, ended_at, kind, state_json
         FROM games WHERE room_id = ?1 AND ended_at IS NULL",
    )
    .bind(room_id)
    .fetch_optional(pool)
    .await
    .expect("get_active_game failed")
}

/// Overwrites a game's freeform state blob — used by games (Shell, 3 Man)
/// that track progress beyond the shared draw/rules tables.
pub async fn set_game_state(pool: &DbPool, game_id: i64, state_json: &str) {
    sqlx::query("UPDATE games SET state_json = ?1 WHERE id = ?2")
        .bind(state_json)
        .bind(game_id)
        .execute(pool)
        .await
        .expect("set_game_state failed");
}

/// Claims the next undrawn card index for player_id and returns it.
/// A double-tap race loses on UNIQUE(game_id, card_index) and retries with
/// the next index. Terminates unconditionally when any unique-violation
/// insert reaches 52 (no more indices to claim).
/// `deck_ranks[next_index]` is written as the draw's rank at insert time —
/// callers pass the game's deck order pre-mapped to ranks so this stays a
/// pure index lookup, no card parsing in the hot loop.
pub async fn insert_draw(
    pool: &DbPool,
    game_id: i64,
    player_id: i64,
    deck_ranks: &[u8],
) -> Result<i64, GameError> {
    loop {
        let (next_index,): (i64,) = sqlx::query_as(
            "SELECT COALESCE(MAX(card_index) + 1, 0) FROM game_draws WHERE game_id = ?1",
        )
        .bind(game_id)
        .fetch_one(pool)
        .await
        .map_err(GameError::from)?;
        if next_index >= 52 {
            return Err(GameError::DeckExhausted);
        }
        let rank = deck_ranks[next_index as usize] as i64;
        let res = sqlx::query(
            "INSERT INTO game_draws (game_id, player_id, card_index, rank) VALUES (?1, ?2, ?3, ?4)",
        )
        .bind(game_id)
        .bind(player_id)
        .bind(next_index)
        .bind(rank)
        .execute(pool)
        .await;
        match res {
            Ok(_) => return Ok(next_index),
            Err(e)
                if e.as_database_error()
                    .is_some_and(|d| d.is_unique_violation()) =>
            {
                continue
            }
            Err(e) => return Err(e.into()),
        }
    }
}

pub async fn get_draws(pool: &DbPool, game_id: i64) -> Vec<DrawRow> {
    sqlx::query_as::<_, DrawRow>(
        "SELECT gd.id, gd.player_id, p.name AS player_name, gd.card_index, gd.spent_at, gd.rank
         FROM game_draws gd JOIN players p ON p.id = gd.player_id
         WHERE gd.game_id = ?1 ORDER BY gd.card_index",
    )
    .bind(game_id)
    .fetch_all(pool)
    .await
    .expect("get_draws failed")
}

/// True only when the draw exists in game_id, belongs to player_id, and is
/// unspent — the game_id guard stops spends against draws from ended games.
pub async fn spend_draw(pool: &DbPool, game_id: i64, draw_id: i64, player_id: i64) -> bool {
    let res = sqlx::query(
        "UPDATE game_draws SET spent_at = datetime('now')
         WHERE id = ?1 AND player_id = ?2 AND game_id = ?3 AND spent_at IS NULL",
    )
    .bind(draw_id)
    .bind(player_id)
    .bind(game_id)
    .execute(pool)
    .await
    .expect("spend_draw failed");
    res.rows_affected() > 0
}

pub async fn end_game(pool: &DbPool, game_id: i64) {
    sqlx::query("UPDATE games SET ended_at = datetime('now') WHERE id = ?1 AND ended_at IS NULL")
        .bind(game_id)
        .execute(pool)
        .await
        .expect("end_game failed");
}

/// Per-player draw totals, most draws first, then name for stable order.
pub async fn draw_counts(pool: &DbPool, game_id: i64) -> Vec<DrawCount> {
    sqlx::query_as::<_, DrawCount>(
        "SELECT p.name, COUNT(*) AS draws
         FROM game_draws gd JOIN players p ON p.id = gd.player_id
         WHERE gd.game_id = ?1
         GROUP BY p.id ORDER BY draws DESC, p.name ASC",
    )
    .bind(game_id)
    .fetch_all(pool)
    .await
    .expect("draw_counts failed")
}

/// Lifetime totals across all rooms — the long-term profile stat.
pub async fn lifetime_counts(pool: &DbPool, player_id: i64) -> (i64, i64) {
    let row: (i64, i64) = sqlx::query_as(
        "SELECT COALESCE(SUM(CASE WHEN kind = 'drink' THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN kind = 'shot'  THEN 1 ELSE 0 END), 0)
         FROM events WHERE player_id = ?1 AND undone_at IS NULL",
    )
    .bind(player_id)
    .fetch_one(pool)
    .await
    .expect("lifetime_counts failed");
    row
}

/// Inserts a house rule for the given draw. Err on the draw already having
/// one (UNIQUE(draw_id) violation) — callers map it to a friendly error.
pub async fn insert_house_rule(
    pool: &DbPool,
    game_id: i64,
    draw_id: i64,
    player_id: i64,
    text: &str,
) -> Result<i64, sqlx::Error> {
    let res = sqlx::query(
        "INSERT INTO game_house_rules (game_id, draw_id, player_id, text) VALUES (?1, ?2, ?3, ?4)",
    )
    .bind(game_id)
    .bind(draw_id)
    .bind(player_id)
    .bind(text)
    .execute(pool)
    .await?;
    Ok(res.last_insert_rowid())
}

pub async fn house_rules(pool: &DbPool, game_id: i64) -> Vec<HouseRule> {
    sqlx::query_as::<_, HouseRule>(
        "SELECT hr.id, hr.draw_id, hr.player_id, p.name AS player_name, hr.text
         FROM game_house_rules hr JOIN players p ON p.id = hr.player_id
         WHERE hr.game_id = ?1 ORDER BY hr.id",
    )
    .bind(game_id)
    .fetch_all(pool)
    .await
    .expect("house_rules failed")
}

/// Number of Kings (rank 13) drawn so far in this game.
pub async fn king_count(pool: &DbPool, game_id: i64) -> i64 {
    let row: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM game_draws WHERE game_id = ?1 AND rank = 13")
            .bind(game_id)
            .fetch_one(pool)
            .await
            .expect("king_count failed");
    row.0
}

/// Name of whoever drew the most recent King, or None if no King has been
/// drawn yet.
pub async fn last_king_drawer(pool: &DbPool, game_id: i64) -> Option<String> {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT p.name FROM game_draws gd JOIN players p ON p.id = gd.player_id
         WHERE gd.game_id = ?1 AND gd.rank = 13
         ORDER BY gd.card_index DESC LIMIT 1",
    )
    .bind(game_id)
    .fetch_optional(pool)
    .await
    .expect("last_king_drawer failed");
    row.map(|(name,)| name)
}

/// Lifetime count of distinct rooms (nights) a player has joined.
pub async fn lifetime_nights(pool: &DbPool, player_id: i64) -> i64 {
    let row: (i64,) =
        sqlx::query_as("SELECT COUNT(DISTINCT room_id) FROM room_players WHERE player_id = ?1")
            .bind(player_id)
            .fetch_one(pool)
            .await
            .expect("lifetime_nights failed");
    row.0
}

/// Lifetime count of Kings drawn by a player, across all games.
pub async fn lifetime_kings(pool: &DbPool, player_id: i64) -> i64 {
    let row: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM game_draws WHERE player_id = ?1 AND rank = 13")
            .bind(player_id)
            .fetch_one(pool)
            .await
            .expect("lifetime_kings failed");
    row.0
}

#[cfg(test)]
pub(crate) async fn test_pool() -> DbPool {
    // max_connections(1): each :memory: connection is a SEPARATE empty db,
    // so the pool must never open a second one.
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    run_migrations(&pool).await;
    pool
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_migrations_are_idempotent() {
        let pool = test_pool().await;
        run_migrations(&pool).await; // second run must not error
    }

    #[tokio::test]
    async fn test_insert_and_get_player() {
        let pool = test_pool().await;
        let id = insert_player(&pool, "hampter", "fakehash").await.unwrap();
        let p = get_player_by_name(&pool, "hampter").await.unwrap();
        assert_eq!(p.id, id);
        assert_eq!(p.pin_hash, "fakehash");
        assert!(!p.created_at.is_empty());
    }

    #[tokio::test]
    async fn test_player_name_is_case_insensitive_unique() {
        let pool = test_pool().await;
        insert_player(&pool, "Hampter", "h1").await.unwrap();
        assert!(insert_player(&pool, "hampter", "h2").await.is_err());
        // Lookup also matches case-insensitively (COLLATE NOCASE on the column).
        assert!(get_player_by_name(&pool, "HAMPTER").await.is_some());
    }

    #[tokio::test]
    async fn test_session_roundtrip_and_expiry() {
        let pool = test_pool().await;
        let pid = insert_player(&pool, "sess", "h").await.unwrap();
        create_session(&pool, "tok-live", pid, "+90 days").await;
        create_session(&pool, "tok-dead", pid, "-1 days").await;

        assert_eq!(get_session_player(&pool, "tok-live").await.unwrap().id, pid);
        assert!(get_session_player(&pool, "tok-dead").await.is_none());
        assert!(get_session_player(&pool, "tok-unknown").await.is_none());

        cleanup_expired_sessions(&pool).await;
        // Live session survives the sweep.
        assert!(get_session_player(&pool, "tok-live").await.is_some());
    }

    async fn seed_room_with_players(pool: &DbPool) -> (i64, i64, i64) {
        let a = insert_player(pool, "alice", "h").await.unwrap();
        let b = insert_player(pool, "bob", "h").await.unwrap();
        let room = crate::rooms::create_room_with_unique_code(pool).await;
        join_room(pool, room.id, a).await;
        join_room(pool, room.id, b).await;
        (room.id, a, b)
    }

    #[tokio::test]
    async fn test_leaderboard_fold_and_order() {
        let pool = test_pool().await;
        let (room, alice, bob) = seed_room_with_players(&pool).await;
        insert_event(&pool, room, alice, "drink").await;
        insert_event(&pool, room, alice, "shot").await;
        insert_event(&pool, room, bob, "drink").await;

        let lb = leaderboard(&pool, room).await;
        assert_eq!(lb.len(), 2);
        assert_eq!(
            (lb[0].name.as_str(), lb[0].drinks, lb[0].shots),
            ("alice", 1, 1)
        );
        assert_eq!(
            (lb[1].name.as_str(), lb[1].drinks, lb[1].shots),
            ("bob", 1, 0)
        );
    }

    #[tokio::test]
    async fn test_members_with_no_events_appear_with_zeros() {
        let pool = test_pool().await;
        let (room, _alice, _bob) = seed_room_with_players(&pool).await;
        let lb = leaderboard(&pool, room).await;
        assert_eq!(lb.len(), 2);
        assert!(lb.iter().all(|r| r.drinks == 0 && r.shots == 0));
    }

    #[tokio::test]
    async fn test_undo_tombstones_latest_only() {
        let pool = test_pool().await;
        let (room, alice, _bob) = seed_room_with_players(&pool).await;
        insert_event(&pool, room, alice, "drink").await;
        insert_event(&pool, room, alice, "shot").await;

        assert!(undo_last_event(&pool, room, alice).await); // kills the shot
        let lb = leaderboard(&pool, room).await;
        let a = lb.iter().find(|r| r.name == "alice").unwrap();
        assert_eq!((a.drinks, a.shots), (1, 0));

        assert!(undo_last_event(&pool, room, alice).await); // kills the drink
        assert!(!undo_last_event(&pool, room, alice).await); // nothing left

        // Rows still exist — tombstoned, not deleted.
        let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM events WHERE room_id = ?1")
            .bind(room)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(total.0, 2);
    }

    #[tokio::test]
    async fn test_lifetime_counts_span_rooms_and_respect_undo() {
        let pool = test_pool().await;
        let alice = insert_player(&pool, "alice", "h").await.unwrap();
        let r1 = crate::rooms::create_room_with_unique_code(&pool).await;
        let r2 = crate::rooms::create_room_with_unique_code(&pool).await;
        join_room(&pool, r1.id, alice).await;
        join_room(&pool, r2.id, alice).await;
        insert_event(&pool, r1.id, alice, "drink").await;
        insert_event(&pool, r2.id, alice, "drink").await;
        insert_event(&pool, r2.id, alice, "shot").await;
        undo_last_event(&pool, r2.id, alice).await; // removes the shot

        assert_eq!(lifetime_counts(&pool, alice).await, (2, 0));
    }

    #[tokio::test]
    async fn test_standard_preset_is_seeded_and_seed_is_idempotent() {
        let pool = test_pool().await;
        run_migrations(&pool).await; // second run must not duplicate the seed
        let presets = list_presets(&pool).await;
        assert_eq!(presets.len(), 1);
        assert_eq!(presets[0].name, "Standard");
        let rules = crate::rules::parse_rules(&presets[0].rules_json);
        assert_eq!(rules, crate::rules::standard_rules());
    }

    #[tokio::test]
    async fn test_preset_crud_roundtrip() {
        let pool = test_pool().await;
        let json = crate::rules::standard_rules_json();
        let id = insert_preset(&pool, "House", &json).await.unwrap();
        assert_eq!(get_preset(&pool, id).await.unwrap().name, "House");
        // Duplicate name rejected.
        assert!(insert_preset(&pool, "House", &json).await.is_err());
        // Update name + rules.
        let mut rules = crate::rules::standard_rules();
        rules[3].title = "Floor".to_string();
        let new_json = serde_json::to_string(&rules).unwrap();
        assert!(update_preset(&pool, id, "House 2", &new_json)
            .await
            .unwrap());
        let got = get_preset(&pool, id).await.unwrap();
        assert_eq!(got.name, "House 2");
        assert_eq!(crate::rules::parse_rules(&got.rules_json)[3].title, "Floor");
        // Update of a missing id reports false.
        assert!(!update_preset(&pool, 9999, "X", &new_json).await.unwrap());
        // Delete.
        assert!(delete_preset(&pool, id).await);
        assert!(get_preset(&pool, id).await.is_none());
        assert!(!delete_preset(&pool, id).await);
    }

    #[tokio::test]
    async fn test_delete_standard_preset_returns_after_migration_rerun() {
        let pool = test_pool().await;
        let standard = &list_presets(&pool).await[0];
        assert!(delete_preset(&pool, standard.id).await);
        assert!(list_presets(&pool).await.is_empty());
        run_migrations(&pool).await; // deploy re-runs migrations
        assert_eq!(list_presets(&pool).await[0].name, "Standard");
    }

    async fn seed_game(pool: &DbPool) -> (i64, i64, i64, i64) {
        let (room, alice, bob) = seed_room_with_players(pool).await;
        let deck = crate::cards::deck_to_string(&crate::cards::shuffled_deck());
        let game = start_game(
            pool,
            room,
            "ring_of_fire",
            &crate::rules::standard_rules_json(),
            &deck,
            None,
        )
        .await
        .unwrap();
        (room, game, alice, bob)
    }

    /// Test-only lookup: a game's deck_order by id, regardless of whether
    /// the game is still active.
    async fn get_active_game_deck(pool: &DbPool, game_id: i64) -> String {
        sqlx::query_as::<_, (String,)>("SELECT deck_order FROM games WHERE id = ?1")
            .bind(game_id)
            .fetch_one(pool)
            .await
            .unwrap()
            .0
    }

    async fn deck_ranks(pool: &DbPool, game_id: i64) -> Vec<u8> {
        crate::cards::parse_deck(&get_active_game_deck(pool, game_id).await)
            .iter()
            .map(|c| c.rank)
            .collect()
    }

    #[tokio::test]
    async fn test_one_active_game_per_room() {
        let pool = test_pool().await;
        let (room, _game, _a, _b) = seed_game(&pool).await;
        let deck = crate::cards::deck_to_string(&crate::cards::shuffled_deck());
        let err = start_game(&pool, room, "ring_of_fire", "[]", &deck, None)
            .await
            .unwrap_err();
        assert!(matches!(err, crate::error::GameError::GameAlreadyActive));
        // Ending frees the room for a new game.
        let game = get_active_game(&pool, room).await.unwrap();
        end_game(&pool, game.id).await;
        assert!(get_active_game(&pool, room).await.is_none());
        assert!(start_game(&pool, room, "ring_of_fire", "[]", &deck, None)
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn test_draws_come_back_in_deck_order() {
        let pool = test_pool().await;
        let (_room, game, alice, bob) = seed_game(&pool).await;
        let ranks = deck_ranks(&pool, game).await;
        assert_eq!(insert_draw(&pool, game, alice, &ranks).await.unwrap(), 0);
        assert_eq!(insert_draw(&pool, game, bob, &ranks).await.unwrap(), 1);
        assert_eq!(insert_draw(&pool, game, alice, &ranks).await.unwrap(), 2);
        let draws = get_draws(&pool, game).await;
        assert_eq!(
            draws.iter().map(|d| d.card_index).collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
        assert_eq!(draws[0].player_name, "alice");
        assert_eq!(draws[1].player_name, "bob");
    }

    #[tokio::test]
    async fn test_double_draw_on_same_index_conflicts_and_retries() {
        let pool = test_pool().await;
        let (_room, game, alice, bob) = seed_game(&pool).await;
        // Simulate alice's in-flight draw landing first on index 0.
        sqlx::query("INSERT INTO game_draws (game_id, player_id, card_index) VALUES (?1, ?2, 0)")
            .bind(game)
            .bind(alice)
            .execute(&pool)
            .await
            .unwrap();
        // Bob's insert_draw must skip to index 1, not fail or duplicate.
        let ranks = deck_ranks(&pool, game).await;
        assert_eq!(insert_draw(&pool, game, bob, &ranks).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_deck_exhaustion() {
        let pool = test_pool().await;
        let (_room, game, alice, _bob) = seed_game(&pool).await;
        let ranks = deck_ranks(&pool, game).await;
        for i in 0..52 {
            assert_eq!(insert_draw(&pool, game, alice, &ranks).await.unwrap(), i);
        }
        let err = insert_draw(&pool, game, alice, &ranks).await.unwrap_err();
        assert!(matches!(err, crate::error::GameError::DeckExhausted));
    }

    #[tokio::test]
    async fn test_spend_only_holder_only_once() {
        let pool = test_pool().await;
        let (_room, game, alice, bob) = seed_game(&pool).await;
        let ranks = deck_ranks(&pool, game).await;
        insert_draw(&pool, game, alice, &ranks).await.unwrap();
        let draw_id = get_draws(&pool, game).await[0].id;

        // Create a second game in a different room to test game_id guard
        let room2 = crate::rooms::create_room_with_unique_code(&pool).await;
        let deck = crate::cards::deck_to_string(&crate::cards::shuffled_deck());
        let game2 = start_game(
            &pool,
            room2.id,
            "ring_of_fire",
            &crate::rules::standard_rules_json(),
            &deck,
            None,
        )
        .await
        .unwrap();

        assert!(!spend_draw(&pool, game, draw_id, bob).await); // not the holder
        assert!(!spend_draw(&pool, game2, draw_id, alice).await); // wrong game
        assert!(spend_draw(&pool, game, draw_id, alice).await); // holder spends
        assert!(!spend_draw(&pool, game, draw_id, alice).await); // already spent
        assert!(!spend_draw(&pool, game, 9999, alice).await); // no such draw
        assert!(get_draws(&pool, game).await[0].spent_at.is_some());
    }

    #[tokio::test]
    async fn test_draw_counts_order_and_totals() {
        let pool = test_pool().await;
        let (_room, game, alice, bob) = seed_game(&pool).await;
        let ranks = deck_ranks(&pool, game).await;
        insert_draw(&pool, game, bob, &ranks).await.unwrap();
        insert_draw(&pool, game, alice, &ranks).await.unwrap();
        insert_draw(&pool, game, bob, &ranks).await.unwrap();
        assert_eq!(
            draw_counts(&pool, game).await,
            vec![
                DrawCount {
                    name: "bob".into(),
                    draws: 2
                },
                DrawCount {
                    name: "alice".into(),
                    draws: 1
                },
            ]
        );
    }

    #[tokio::test]
    async fn test_migration_003_adds_columns_and_is_idempotent() {
        let pool = test_pool().await;
        run_migrations(&pool).await; // second run must not error
                                     // Columns exist with defaults.
        let g = seed_game(&pool).await;
        let game = get_active_game(&pool, g.0).await.unwrap();
        assert_eq!(game.kind, "ring_of_fire");
        assert!(game.state_json.is_none());
    }

    #[tokio::test]
    async fn test_rank_backfill_is_idempotent_and_correct() {
        let pool = test_pool().await;
        let (_room, game, alice, _bob) = seed_game(&pool).await;
        let deck = crate::cards::parse_deck(&get_active_game_deck(&pool, game).await);
        let ranks: Vec<u8> = deck.iter().map(|c| c.rank).collect();
        insert_draw(&pool, game, alice, &ranks).await.unwrap();
        // Simulate a pre-003 row: null out the rank, then re-run migrations.
        sqlx::query("UPDATE game_draws SET rank = NULL")
            .execute(&pool)
            .await
            .unwrap();
        run_migrations(&pool).await;
        run_migrations(&pool).await; // idempotent
        let draws = get_draws(&pool, game).await;
        assert_eq!(draws[0].rank, deck[0].rank as i64);
    }

    #[tokio::test]
    async fn test_house_rule_one_per_draw() {
        let pool = test_pool().await;
        let (room, game, alice, _bob) = seed_game(&pool).await;
        let deck =
            crate::cards::parse_deck(&get_active_game(&pool, room).await.unwrap().deck_order);
        let ranks: Vec<u8> = deck.iter().map(|c| c.rank).collect();
        insert_draw(&pool, game, alice, &ranks).await.unwrap();
        let draw_id = get_draws(&pool, game).await[0].id;
        assert!(insert_house_rule(&pool, game, draw_id, alice, "no names")
            .await
            .is_ok());
        assert!(insert_house_rule(&pool, game, draw_id, alice, "again")
            .await
            .is_err());
        let rules = house_rules(&pool, game).await;
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].player_name, "alice");
    }

    #[tokio::test]
    async fn test_lifetime_nights_and_kings() {
        let pool = test_pool().await;
        let alice = insert_player(&pool, "alice", "h").await.unwrap();
        let r1 = crate::rooms::create_room_with_unique_code(&pool).await;
        let r2 = crate::rooms::create_room_with_unique_code(&pool).await;
        join_room(&pool, r1.id, alice).await;
        join_room(&pool, r2.id, alice).await;
        assert_eq!(lifetime_nights(&pool, alice).await, 2);

        // Rig a deck whose first card is a King, draw it.
        let mut deck = crate::cards::shuffled_deck();
        let king_pos = deck.iter().position(|c| c.rank == 13).unwrap();
        deck.swap(0, king_pos);
        let deck_str = crate::cards::deck_to_string(&deck);
        let game = start_game(
            &pool,
            r1.id,
            "ring_of_fire",
            &crate::rules::standard_rules_json(),
            &deck_str,
            None,
        )
        .await
        .unwrap();
        let ranks: Vec<u8> = deck.iter().map(|c| c.rank).collect();
        insert_draw(&pool, game, alice, &ranks).await.unwrap();

        assert_eq!(king_count(&pool, game).await, 1);
        assert_eq!(last_king_drawer(&pool, game).await, Some("alice".into()));
        assert_eq!(lifetime_kings(&pool, alice).await, 1);
    }

    #[tokio::test]
    async fn test_insert_events_bulk_counts_rows() {
        let pool = test_pool().await;
        let (room, alice, _bob) = seed_room_with_players(&pool).await;
        insert_events_bulk(&pool, room, alice, "drink", 4).await;
        let lb = leaderboard(&pool, room).await;
        assert_eq!(lb.iter().find(|r| r.name == "alice").unwrap().drinks, 4);
    }

    #[tokio::test]
    async fn test_end_room_ends_active_game() {
        let pool = test_pool().await;
        let (room, game, _a, _b) = seed_game(&pool).await;
        end_room(&pool, room).await;
        let row: (Option<String>,) = sqlx::query_as("SELECT ended_at FROM games WHERE id = ?1")
            .bind(game)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert!(row.0.is_some());
    }

    #[tokio::test]
    async fn test_end_inactive_rooms_ends_their_games() {
        let pool = test_pool().await;
        let (room, game, _a, _b) = seed_game(&pool).await;
        // Backdate the room 13 hours, mirroring rooms::test_end_inactive_rooms.
        sqlx::query(
            "UPDATE rooms SET last_activity_at = datetime('now', '-13 hours') WHERE id = ?1",
        )
        .bind(room)
        .execute(&pool)
        .await
        .unwrap();
        let ended = end_inactive_rooms(&pool, 12).await;
        assert_eq!(ended, vec![room]);
        let row: (Option<String>,) = sqlx::query_as("SELECT ended_at FROM games WHERE id = ?1")
            .bind(game)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert!(row.0.is_some());
    }

    #[tokio::test]
    async fn test_room_members_ordered_by_join() {
        let pool = test_pool().await;
        let (room, alice, bob) = seed_room_with_players(&pool).await;
        let members = room_members(&pool, room).await;
        assert_eq!(
            members.iter().map(|m| m.id).collect::<Vec<_>>(),
            vec![alice, bob]
        );
    }
}
