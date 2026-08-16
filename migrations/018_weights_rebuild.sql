-- Migration 018: re-key weights by user.
--
-- A *rebuild*, not an ALTER: SQLite cannot change a primary key, and this
-- table encodes "one user" directly in its own — `date` was the PK, so two
-- people could not weigh in on the same day.
--
-- ⚠ NOT SELF-IDEMPOTENT. Run it twice and the second pass copies every user's
-- rows into a fresh table as user 1 and drops the original. The `let _ =`
-- duplicate-column tolerance the other migrations rely on cannot express
-- "already done" for a rebuild — a re-run does not error, it silently
-- destroys. `run_migrations` guards this file on `column_exists(weights,
-- user_id)`.
--
-- Its own file, with its own guard, rather than sharing one with the targets
-- rebuild: DDL statements auto-commit individually, so a batch that renamed
-- `weights` and then failed would leave a guard reporting "already done" over
-- a half-migrated schema.
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
