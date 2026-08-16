-- Migration 019: split per-user preferences off the shared food catalog.
--
-- `food_items` stays shared — that is the point of the catalog, and barcode /
-- OpenFoodFacts lookups compound across everyone who uses it. But migration 010
-- put three *personal* opinions on that shared row: whether the food is a
-- favourite, the portion you usually take, and your custom portion sizes. Two
-- users would overwrite each other's answers on every toggle.
--
-- ⚠ NOT SELF-IDEMPOTENT (the DROP COLUMNs): guarded in `run_migrations` by a
-- `column_exists(food_items, is_favourite)` check.
CREATE TABLE IF NOT EXISTS user_food_prefs (
    user_id           INTEGER NOT NULL,
    food_item_id      INTEGER NOT NULL,
    is_favourite      INTEGER NOT NULL DEFAULT 0,
    default_portion_g REAL,
    custom_portions   TEXT NOT NULL DEFAULT '',
    PRIMARY KEY (user_id, food_item_id)
);

-- Carry the owner's existing opinions over. Only rows that actually say
-- something are copied — a food nobody favourited and never gave a portion to
-- needs no preference row, and the readers left-join so a missing row reads as
-- "no opinion" rather than as absent data.
INSERT OR IGNORE INTO user_food_prefs (user_id, food_item_id, is_favourite, default_portion_g, custom_portions)
    SELECT 1, id, is_favourite, default_portion_g, custom_portions
    FROM food_items
    WHERE is_favourite = 1 OR default_portion_g IS NOT NULL OR custom_portions != '';

ALTER TABLE food_items DROP COLUMN is_favourite;
ALTER TABLE food_items DROP COLUMN default_portion_g;
ALTER TABLE food_items DROP COLUMN custom_portions;
