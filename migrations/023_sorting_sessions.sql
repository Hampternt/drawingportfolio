-- Migration 023: sorting sessions — the crate sort / van-load board.
--
-- One row per sorting session: the JSON document produced for a route (stops,
-- crate manifest, pallet stacks, van config, loading plan, pick sequence),
-- stored whole in `payload`. Keeping it as one blob rather than shredding it
-- into eight tables is deliberate — nothing queries *across* sessions, the
-- document is written once and read whole, and a schema that mirrors the
-- generator's output cannot drift from it.
--
-- Progress deliberately does NOT live in that blob. `sorting_step_state`
-- carries one row per completed pick step, which makes a tick a single INSERT
-- rather than a read-modify-write of the whole document. That is the
-- difference between two taps a second apart both landing, and the second
-- silently overwriting the first — a real risk when the thing tapping is a
-- gloved hand on a tablet that may also be retrying a request it thinks failed.
--
-- `total_steps`/`total_crates` are denormalised out of the payload at insert
-- time so the session list can draw a progress bar without parsing every blob
-- it lists.
--
-- CREATE ... IF NOT EXISTS throughout: this file is self-idempotent, so
-- run_migrations() calls it with .expect() rather than the `let _ =`
-- duplicate-column tolerance the ALTER migrations lean on.

CREATE TABLE IF NOT EXISTS sorting_sessions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    route_name TEXT NOT NULL DEFAULT '',
    session_date TEXT NOT NULL DEFAULT '',
    total_steps INTEGER NOT NULL DEFAULT 0,
    total_crates INTEGER NOT NULL DEFAULT 0,
    payload TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- The session list's only query: this user's sessions, newest route first.
CREATE INDEX IF NOT EXISTS idx_sorting_sessions_user
    ON sorting_sessions (user_id, session_date DESC, id DESC);

-- One row per completed step. Absent row = not done, so an unticked step costs
-- nothing and a reset is a single DELETE. The composite primary key makes the
-- tick idempotent: INSERT OR IGNORE, and a double-tap is a no-op rather than a
-- duplicate.
--
-- No user_id column here on purpose. Ownership lives on the parent row, and
-- every write below re-checks it there with an EXISTS guard, so there is only
-- one place a session can change hands and no way for the two to disagree.
CREATE TABLE IF NOT EXISTS sorting_step_state (
    session_id INTEGER NOT NULL,
    step INTEGER NOT NULL,
    completed_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (session_id, step)
);
