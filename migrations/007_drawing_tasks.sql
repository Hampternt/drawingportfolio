CREATE TABLE IF NOT EXISTS task_images (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    title      TEXT NOT NULL DEFAULT '',
    image_url  TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS drawing_tasks (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    image_id   INTEGER NOT NULL REFERENCES task_images(id) ON DELETE CASCADE,
    title      TEXT NOT NULL,
    prompt     TEXT NOT NULL DEFAULT '',
    subject    TEXT NOT NULL DEFAULT '',
    difficulty TEXT NOT NULL DEFAULT 'medium',
    task_type  TEXT NOT NULL DEFAULT '',
    completed  INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
