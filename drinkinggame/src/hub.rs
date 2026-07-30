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
            .or_insert_with(|| broadcast::channel(32).0)
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
    }
}
