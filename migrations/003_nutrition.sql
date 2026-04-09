CREATE TABLE IF NOT EXISTS food_items (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    name          TEXT NOT NULL,
    brand         TEXT NOT NULL DEFAULT '',
    barcode       TEXT,
    calories      REAL NOT NULL DEFAULT 0,
    protein       REAL NOT NULL DEFAULT 0,
    carbs         REAL NOT NULL DEFAULT 0,
    fat           REAL NOT NULL DEFAULT 0,
    fiber         REAL NOT NULL DEFAULT 0,
    sugar         REAL NOT NULL DEFAULT 0,
    sodium        REAL NOT NULL DEFAULT 0,
    saturated_fat REAL NOT NULL DEFAULT 0,
    image_url     TEXT NOT NULL DEFAULT '',
    created_at    TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS meal_entries (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    food_item_id INTEGER NOT NULL REFERENCES food_items(id) ON DELETE CASCADE,
    date         TEXT NOT NULL,
    grams        REAL NOT NULL DEFAULT 100,
    created_at   TEXT NOT NULL DEFAULT (datetime('now'))
);
