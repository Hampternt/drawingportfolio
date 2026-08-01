CREATE TABLE IF NOT EXISTS targets (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    calories REAL NOT NULL,
    protein REAL NOT NULL,
    carbs REAL NOT NULL,
    fat REAL NOT NULL
);
INSERT OR IGNORE INTO targets (id, calories, protein, carbs, fat) VALUES (1, 2400, 165, 260, 72);
