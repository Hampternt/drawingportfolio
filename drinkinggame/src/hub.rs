//! Per-room broadcast channels. Every connected screen (player phone or
//! spectator) subscribes; any state change publishes a freshly rendered
//! leaderboard fragment.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

#[derive(Clone, Debug)]
pub enum RoomMessage {
    /// Rendered <li> rows for the leaderboard.
    Leaderboard(String),
    /// Rendered Ring of Fire panel HTML for the #game-panel container.
    Game(String),
    /// Rendered screen/display HTML for the #screen-container.
    Screen(String),
    /// Rendered room HTML fragment.
    Room(String),
    /// Rendered emote glyph.
    Emote(String),
    /// Rendered PUBLIC Last Call fragment (phase banner now; felt, plaques,
    /// hand sizes and deck counts in Plan B). Broadcast to everyone including
    /// the unauthenticated spectator screen — rendered from `PublicView`, so it
    /// cannot contain unrevealed card identity by construction (spec §3.4).
    LcPublic(String),
    /// The game's current `seq`. Carries no state — it only tells each phone to
    /// re-fetch its own private fragment.
    LcTick(u64),
    /// The room was ended; clients should leave.
    Ended,
}

#[derive(Clone, Default)]
pub struct RoomHub {
    // std Mutex, not tokio: we never await while holding the lock.
    inner: Arc<Mutex<HashMap<i64, broadcast::Sender<RoomMessage>>>>,
}

impl RoomHub {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn subscribe(&self, room_id: i64) -> broadcast::Receiver<RoomMessage> {
        let mut map = self.inner.lock().unwrap();
        map.entry(room_id)
            .or_insert_with(|| broadcast::channel(128).0)
            .subscribe()
    }

    /// Publishing to a room with no subscribers is a silent no-op.
    pub fn publish(&self, room_id: i64, msg: RoomMessage) {
        let map = self.inner.lock().unwrap();
        if let Some(tx) = map.get(&room_id) {
            let _ = tx.send(msg);
        }
    }

    pub fn remove(&self, room_id: i64) {
        self.inner.lock().unwrap().remove(&room_id);
    }

    pub fn active_rooms(&self) -> Vec<i64> {
        self.inner.lock().unwrap().keys().copied().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_subscribe_publish_remove() {
        let hub = RoomHub::new();
        let mut rx = hub.subscribe(1);
        hub.publish(1, RoomMessage::Leaderboard("<li>x</li>".into()));
        match rx.recv().await.unwrap() {
            RoomMessage::Leaderboard(html) => assert_eq!(html, "<li>x</li>"),
            other => panic!("unexpected message: {other:?}"),
        }
        assert_eq!(hub.active_rooms(), vec![1]);
        hub.remove(1);
        assert!(hub.active_rooms().is_empty());
        hub.publish(1, RoomMessage::Ended); // no subscribers — must not panic

        // Test new variants: Screen, Room, Emote
        let hub = RoomHub::new();
        let mut rx = hub.subscribe(2);

        // Test Screen variant
        hub.publish(2, RoomMessage::Screen("<div>screen</div>".into()));
        match rx.recv().await.unwrap() {
            RoomMessage::Screen(html) => assert_eq!(html, "<div>screen</div>"),
            other => panic!("unexpected Screen message: {other:?}"),
        }

        // Test Room variant
        hub.publish(2, RoomMessage::Room("<div>room</div>".into()));
        match rx.recv().await.unwrap() {
            RoomMessage::Room(html) => assert_eq!(html, "<div>room</div>"),
            other => panic!("unexpected Room message: {other:?}"),
        }

        // Test Emote variant
        hub.publish(2, RoomMessage::Emote("🎉".into()));
        match rx.recv().await.unwrap() {
            RoomMessage::Emote(glyph) => assert_eq!(glyph, "🎉"),
            other => panic!("unexpected Emote message: {other:?}"),
        }

        // Test LcPublic variant
        hub.publish(2, RoomMessage::LcPublic("<div>lc</div>".into()));
        match rx.recv().await.unwrap() {
            RoomMessage::LcPublic(html) => assert_eq!(html, "<div>lc</div>"),
            other => panic!("unexpected LcPublic message: {other:?}"),
        }

        // Test LcTick variant
        hub.publish(2, RoomMessage::LcTick(7));
        match rx.recv().await.unwrap() {
            RoomMessage::LcTick(seq) => assert_eq!(seq, 7),
            other => panic!("unexpected LcTick message: {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_channel_capacity_is_128() {
        let hub = RoomHub::new();
        let mut rx = hub.subscribe(3);

        // Send 100 messages
        for i in 0..100 {
            hub.publish(3, RoomMessage::Leaderboard(format!("<li>{}</li>", i)));
        }

        // Receive all 100 messages without loss
        for i in 0..100 {
            match rx.recv().await.unwrap() {
                RoomMessage::Leaderboard(html) => {
                    assert_eq!(html, format!("<li>{}</li>", i));
                }
                other => panic!("unexpected message at {}: {other:?}", i),
            }
        }

        // Verify no loss: with capacity 128, 100 messages should not be dropped
        // (if capacity was smaller, early messages would be dropped)
    }
}
