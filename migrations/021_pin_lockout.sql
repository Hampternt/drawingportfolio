-- Migration 021: PIN brute-force lockout.
--
-- A PIN is a small secret on a public endpoint. Argon2 makes each guess cost
-- tens of milliseconds, but a 4-digit PIN is only 10,000 guesses — without a
-- lockout, Argon2 decides how long the attack takes, not whether it works.
--
-- Counted in the database rather than in memory on purpose: an attacker who
-- can make the process restart would otherwise reset their own budget, and the
-- portfolio restarts on every deploy.
ALTER TABLE users ADD COLUMN failed_pin_attempts INTEGER NOT NULL DEFAULT 0;
ALTER TABLE users ADD COLUMN locked_until TEXT;
