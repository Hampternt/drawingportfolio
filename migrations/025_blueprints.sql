-- Migration 025: Blueprints — the owner's projects and the plans inside them.
--
-- A project (say `drawingportfolio`) holds one plan per feature or system.
-- Each plan is one JSON document stored whole in `payload`, the
-- `sorting_sessions.payload` pattern: the document is written and read whole,
-- nothing queries across plans, and a schema that mirrors the editor's model
-- cannot drift from it. The server parses it only to validate a save and to
-- render the outline; it stores the text exactly as the browser sent it, so
-- key order (a port's row on its node) survives a reload.
--
-- `rev` is the plan's version. Every save sends the rev it started from
-- (`If-Match`), and the save is one compare-and-set UPDATE ... WHERE rev = ?,
-- so a stale save changes nothing and is refused with 412. There will be two
-- writers, the browser and (slice S5) the AI, so last-write-wins is out. The
-- revisions table that keeps every accepted write is slice S6, not this one.
--
-- `user_id` sits on the project only. A plan's ownership is its project's, and
-- every query that reaches a plan joins through the project and checks it
-- there, so a plan changes hands in exactly one place. Only the owner reaches
-- these routes today; the scoping is what keeps it that way if that changes.
--
-- Names are unique within their parent, because the AI path will ask for a
-- project and a plan by name. No foreign keys: the pool does not turn
-- `PRAGMA foreign_keys` on, so deleting a project deletes its plans in the same
-- transaction instead (db::delete_blueprint_project).
--
-- CREATE ... IF NOT EXISTS throughout, so this file is self-idempotent and
-- run_migrations() calls it with .expect().

CREATE TABLE IF NOT EXISTS blueprint_projects (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE (user_id, name)
);

CREATE TABLE IF NOT EXISTS blueprint_plans (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    payload TEXT NOT NULL,
    rev INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE (project_id, name)
);
