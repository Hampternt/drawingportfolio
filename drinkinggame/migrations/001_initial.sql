-- Drinking game v1. All timestamps are ISO8601 TEXT (portfolio convention).
CREATE TABLE IF NOT EXISTS players (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE COLLATE NOCASE,
    pin_hash TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS sessions (
    id TEXT PRIMARY KEY,
    player_id INTEGER NOT NULL REFERENCES players(id),
    expires_at TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS rooms (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    code TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    last_activity_at TEXT NOT NULL DEFAULT (datetime('now')),
    ended_at TEXT
);

-- Join codes are unique only among OPEN rooms; ended rooms free their code.
CREATE UNIQUE INDEX IF NOT EXISTS idx_rooms_open_code
    ON rooms(code) WHERE ended_at IS NULL;

CREATE TABLE IF NOT EXISTS room_players (
    room_id INTEGER NOT NULL REFERENCES rooms(id),
    player_id INTEGER NOT NULL REFERENCES players(id),
    joined_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (room_id, player_id)
);

-- Append-only. Undo is a tombstone (undone_at), never a DELETE.
CREATE TABLE IF NOT EXISTS events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    room_id INTEGER NOT NULL REFERENCES rooms(id),
    player_id INTEGER NOT NULL REFERENCES players(id),
    kind TEXT NOT NULL CHECK (kind IN ('drink', 'shot')),
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    undone_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_events_room ON events(room_id, player_id);
