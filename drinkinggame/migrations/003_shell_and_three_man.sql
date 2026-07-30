-- Shell + 3 Man. House rules typed after drawing a Jack; draw_id UNIQUE
-- makes it one rule per Jack, server-verifiable.
CREATE TABLE IF NOT EXISTS game_house_rules (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    game_id INTEGER NOT NULL REFERENCES games(id),
    draw_id INTEGER NOT NULL UNIQUE REFERENCES game_draws(id),
    player_id INTEGER NOT NULL REFERENCES players(id),
    text TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_house_rules_game ON game_house_rules(game_id);
