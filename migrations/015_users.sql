-- Migration 015: real user identity.
--
-- Until now the portfolio had exactly one implicit user: whoever held a
-- passkey was "the admin", and every nutrition row belonged to them by
-- assumption rather than by column. Multi-user fitness needs a real identity,
-- and art-portfolio admin becomes a *grantable permission* rather than a
-- synonym for "logged in".
--
-- pin_hash is nullable on purpose: the owner authenticates with a passkey and
-- may never set a PIN, while a member created from the management page has a
-- PIN and no passkey. Neither is the odd case.
CREATE TABLE IF NOT EXISTS users (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    name       TEXT NOT NULL UNIQUE COLLATE NOCASE,
    pin_hash   TEXT,
    is_owner   INTEGER NOT NULL DEFAULT 0,
    is_admin   INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Exactly one owner, enforced by the database rather than by convention.
-- A partial index over the flag lets any number of rows hold is_owner = 0
-- while making a second is_owner = 1 an INSERT error. The owner is the one
-- account that cannot be demoted or deleted, so this is the invariant that
-- stops the site being locked out of its own admin.
CREATE UNIQUE INDEX IF NOT EXISTS idx_users_single_owner
    ON users(is_owner) WHERE is_owner = 1;

-- Seed the owner as id 1. Every pre-existing row in this database belongs to
-- this user, and migration 016 backfills to that literal id. The name is a
-- placeholder — renaming is part of the account page in pack 4.
INSERT OR IGNORE INTO users (id, name, is_owner, is_admin) VALUES (1, 'admin', 1, 1);
