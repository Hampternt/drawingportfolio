-- The ON DELETE CASCADE clauses do not fire — the pool never sets
-- PRAGMA foreign_keys (see Global Constraints). Deletes clean their own join
-- rows in Rust, in transactions.
--
-- idx_posts_visibility_created arrives here even though the feed's
-- OR-with-a-parameter predicate cannot use it — the spec ships it for the
-- subquery-driven shapes this migration introduces. It changes no behaviour.
CREATE TABLE IF NOT EXISTS collections (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    slug TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE TABLE IF NOT EXISTS post_collections (
    post_id INTEGER NOT NULL REFERENCES posts(id) ON DELETE CASCADE,
    collection_id INTEGER NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
    PRIMARY KEY (post_id, collection_id)
);
CREATE TABLE IF NOT EXISTS tags (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE
);
CREATE TABLE IF NOT EXISTS post_tags (
    post_id INTEGER NOT NULL REFERENCES posts(id) ON DELETE CASCADE,
    tag_id INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    PRIMARY KEY (post_id, tag_id)
);
CREATE INDEX IF NOT EXISTS idx_post_tags_tag ON post_tags(tag_id);
CREATE INDEX IF NOT EXISTS idx_posts_visibility_created
    ON posts(visibility, created_at DESC);
