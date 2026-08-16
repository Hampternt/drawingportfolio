-- Migration 019: re-key targets by user.
--
-- One row per user instead of one row, full stop — the old table was a single
-- row pinned by `CHECK (id = 1)`. A rebuild for the same reason as 018: the
-- primary key itself is what has to change.
--
-- The existing row is the owner's. Everyone else falls back to the defaults in
-- `get_targets` until they set their own, so there is no per-user seeding pass
-- here and a brand-new account reads the same numbers the single-user version
-- shipped with.
--
-- ⚠ NOT SELF-IDEMPOTENT — guarded in `run_migrations` on
-- `column_exists(targets, user_id)`. The old schema has `id`, the new one has
-- `user_id`, so the check is an exact discriminator between the two.
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
