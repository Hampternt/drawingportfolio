-- Migration 016: bind sessions and passkeys to a user.
--
-- NOT NULL DEFAULT 1 *is* the backfill: SQLite writes the default into every
-- existing row as it adds the column, so the session you are currently logged
-- in with and the passkey you registered both survive the migration and belong
-- to the owner seeded by 015. No separate UPDATE pass, no window where a row
-- has no owner.
--
-- No REFERENCES clause: the pool does not enable foreign_keys (see 014), so it
-- would be documentation only — and SQLite refuses ADD COLUMN with both a
-- REFERENCES clause and a non-NULL default if enforcement is ever switched on.
ALTER TABLE sessions ADD COLUMN user_id INTEGER NOT NULL DEFAULT 1;
ALTER TABLE passkey_credentials ADD COLUMN user_id INTEGER NOT NULL DEFAULT 1;
