//! Join-code generation. Codes are 4 letters from an alphabet with I, L and
//! O removed — they're ambiguous on a phone screen at 1am.

use rand::Rng;

use crate::db::{self, DbPool};
use crate::models::Room;

pub const CODE_ALPHABET: &[u8] = b"ABCDEFGHJKMNPQRSTUVWXYZ";
pub const CODE_LEN: usize = 4;

pub fn gen_room_code() -> String {
    let mut rng = rand::thread_rng();
    (0..CODE_LEN)
        .map(|_| CODE_ALPHABET[rng.gen_range(0..CODE_ALPHABET.len())] as char)
        .collect()
}

/// Insert with a fresh code, retrying on collision with an open room (the
/// partial unique index rejects duplicates among ended_at IS NULL rows).
/// 23^4 = 279,841 codes — collisions are rare, 20 retries is generous.
pub async fn create_room_with_unique_code(pool: &DbPool) -> Room {
    for _ in 0..20 {
        let code = gen_room_code();
        if let Ok(id) = db::insert_room(pool, &code).await {
            return db::get_room_by_id(pool, id)
                .await
                .expect("room row must exist after insert");
        }
    }
    panic!("could not find a free room code after 20 attempts");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_code_shape() {
        for _ in 0..100 {
            let code = gen_room_code();
            assert_eq!(code.len(), CODE_LEN);
            assert!(code.bytes().all(|b| CODE_ALPHABET.contains(&b)));
        }
    }

    #[tokio::test]
    async fn test_create_join_end_lifecycle() {
        let pool = crate::db::test_pool().await;
        let room = create_room_with_unique_code(&pool).await;
        assert!(db::get_open_room(&pool, &room.code).await.is_some());

        // Same code can't be inserted while the room is open...
        assert!(db::insert_room(&pool, &room.code).await.is_err());
        // ...but is freed once the room ends.
        db::end_room(&pool, room.id).await;
        assert!(db::get_open_room(&pool, &room.code).await.is_none());
        assert!(db::insert_room(&pool, &room.code).await.is_ok());
    }

    #[tokio::test]
    async fn test_join_is_idempotent() {
        let pool = crate::db::test_pool().await;
        let pid = db::insert_player(&pool, "j", "h").await.unwrap();
        let room = create_room_with_unique_code(&pool).await;
        db::join_room(&pool, room.id, pid).await;
        db::join_room(&pool, room.id, pid).await; // second join: no error, no dup
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM room_players WHERE room_id = ?1")
            .bind(room.id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count.0, 1);
    }

    #[tokio::test]
    async fn test_end_inactive_rooms() {
        let pool = crate::db::test_pool().await;
        let stale = create_room_with_unique_code(&pool).await;
        let fresh = create_room_with_unique_code(&pool).await;
        // Backdate the stale room 13 hours.
        sqlx::query(
            "UPDATE rooms SET last_activity_at = datetime('now', '-13 hours') WHERE id = ?1",
        )
        .bind(stale.id)
        .execute(&pool)
        .await
        .unwrap();

        let ended = db::end_inactive_rooms(&pool, 12).await;
        assert_eq!(ended, vec![stale.id]);
        assert!(db::get_open_room(&pool, &stale.code).await.is_none());
        assert!(db::get_open_room(&pool, &fresh.code).await.is_some());
    }
}
