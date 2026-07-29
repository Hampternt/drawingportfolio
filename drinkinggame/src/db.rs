use crate::models::{LeaderboardRow, Player, Room, RulePreset};
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
    // Seed guard: recreate the Standard preset only if missing, so deleting
    // it is permitted but it returns on next deploy (accepted v1 quirk).
    sqlx::query("INSERT OR IGNORE INTO rule_presets (name, rules_json) VALUES ('Standard', ?1)")
        .bind(crate::rules::standard_rules_json())
        .execute(pool)
        .await
        .expect("standard preset seed failed");
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
    ids.into_iter().map(|(id,)| id).collect()
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
        "SELECT p.name,
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
}
