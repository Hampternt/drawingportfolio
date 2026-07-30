-- Ring of Fire. All timestamps ISO8601 TEXT (portfolio convention).

CREATE TABLE IF NOT EXISTS rule_presets (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    rules_json TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS games (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    room_id INTEGER NOT NULL REFERENCES rooms(id),
    -- Snapshot copied from the preset at start; editing a preset never
    -- mutates a running game.
    rules_json TEXT NOT NULL,
    -- The full shuffled deck as text ("AS,2H,..."): ~150 bytes beats an RNG
    -- seed, whose replay would couple correctness to the RNG never changing.
    deck_order TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    ended_at TEXT
);

-- One active game per room.
CREATE UNIQUE INDEX IF NOT EXISTS idx_games_one_active
    ON games(room_id) WHERE ended_at IS NULL;

CREATE TABLE IF NOT EXISTS game_draws (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    game_id INTEGER NOT NULL REFERENCES games(id),
    player_id INTEGER NOT NULL REFERENCES players(id),
    card_index INTEGER NOT NULL,
    drawn_at TEXT NOT NULL DEFAULT (datetime('now')),
    -- Tombstone for held cards, mirroring events.undone_at.
    spent_at TEXT,
    -- Double-tap race: the loser gets a constraint conflict, not a dupe.
    UNIQUE (game_id, card_index)
);

CREATE INDEX IF NOT EXISTS idx_game_draws_game ON game_draws(game_id);
