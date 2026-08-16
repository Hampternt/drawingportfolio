-- Migration 018: re-key weights and targets by user.
--
-- Both need a *rebuild*, not an ALTER: SQLite cannot change a primary key, and
-- these two encode "one user" directly in theirs — `weights.date` is the PK, so
-- two people cannot weigh in on the same day, and `targets` is a single row
-- pinned by `CHECK (id = 1)`.
--
-- ⚠ THIS FILE IS NOT SELF-IDEMPOTENT. Run it twice and the second pass copies
-- every user's rows into a fresh table as user 1 and drops the original. The
-- `let _ =` duplicate-column tolerance the other migrations rely on cannot
-- express "already done" for a rebuild — a re-run does not error, it silently
-- destroys. `run_migrations` therefore guards this file behind a
-- `column_exists(weights, user_id)` check, and `test_migrations_are_idempotent
-- _across_boots` covers the second-boot path.
CREATE TABLE weights_rebuilt (
    user_id    INTEGER NOT NULL,
    date       TEXT NOT NULL,
    kg         REAL NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (user_id, date)
);
INSERT INTO weights_rebuilt (user_id, date, kg, created_at)
    SELECT 1, date, kg, created_at FROM weights;
DROP TABLE weights;
ALTER TABLE weights_rebuilt RENAME TO weights;

-- Targets: one row per user instead of one row, full stop. The old row is the
-- owner's; everyone else falls back to the defaults in `get_targets` until they
-- set their own, so there is no seeding pass here.
CREATE TABLE targets_rebuilt (
    user_id  INTEGER PRIMARY KEY,
    calories REAL NOT NULL,
    protein  REAL NOT NULL,
    carbs    REAL NOT NULL,
    fat      REAL NOT NULL
);
INSERT INTO targets_rebuilt (user_id, calories, protein, carbs, fat)
    SELECT 1, calories, protein, carbs, fat FROM targets WHERE id = 1;
DROP TABLE targets;
ALTER TABLE targets_rebuilt RENAME TO targets;
