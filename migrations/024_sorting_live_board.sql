-- Migration 024: the live board — an action log, and the driver's loading rules.
--
-- The board this route now renders is not a checklist. There is no fixed list
-- of moves to tick off: the driver says which door a customer is being packed
-- at, pushes crates in a few at a time, tops up somebody else's stack, closes
-- an order out. What that produces is a van — eighteen positions and five
-- packing spots, each holding a stack of somebody's crates — and the board
-- decides where the next push may legally land from that state.
--
-- So the thing to store is what the driver did, in order. `sorting_actions` is
-- append-only and the board replays it to get the van back. Two reasons it is
-- a log rather than a snapshot of the van:
--
--  * A snapshot is a read-modify-write, and the whole point of the previous
--    design was avoiding one. Two taps a second apart both have to land. An
--    append cannot lose the first to the second.
--  * `seq` is the client's own counter, and the primary key is (session, seq),
--    so a retried request is an INSERT OR IGNORE that changes nothing. The
--    tablet queues unsent actions and replays them when the radio comes back;
--    replaying one that already arrived has to be a no-op, not a second push.
--
-- Undo deletes from a sequence number upward — the only DELETE in normal use —
-- which is exactly "put the board back the way it was one tap ago".
--
-- `crates` and `positions` are the totals *after* the action, sent by the
-- client that already computed them for the screen. They are here so the
-- session list can draw a progress bar from one row per session rather than
-- replaying every log it lists. A stale value costs a wrong number in a
-- progress bar and nothing else; the board itself never reads them.
--
-- No user_id column, for the same reason `sorting_step_state` has none:
-- ownership lives on the parent session row and every write re-checks it
-- there, so a session changes hands in exactly one place.
CREATE TABLE IF NOT EXISTS sorting_actions (
    session_id INTEGER NOT NULL,
    seq        INTEGER NOT NULL,
    kind       TEXT NOT NULL,
    payload    TEXT NOT NULL DEFAULT '{}',
    crates     INTEGER NOT NULL DEFAULT 0,
    positions  INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (session_id, seq)
);

-- The van's shape and the rules the board loads by: rows, stack height, how far
-- the side door reaches, how many packing spots, the stability limit, and the
-- order the board reaches for a home in.
--
-- Keyed by user, not by session, and that is the decision worth naming. A van
-- does not change shape between mornings, and a driver who picks up a different
-- tablet should not have to set it up again — which is also why this is not
-- localStorage any more. Only deviations from the defaults are stored, so a
-- default that changes in a later build reaches everybody who never touched it,
-- and "restore defaults" deletes the row rather than writing the defaults into
-- it.
CREATE TABLE IF NOT EXISTS sorting_rules (
    user_id    INTEGER PRIMARY KEY,
    payload    TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
