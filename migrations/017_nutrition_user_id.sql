-- Migration 017: give meal entries and recipes an owner.
--
-- `NOT NULL DEFAULT 1` *is* the backfill — SQLite writes the default into
-- every existing row as it adds the column, so the whole existing food log
-- becomes the owner's in one statement, with no window where a row has no
-- owner.
--
-- `recipe_items` deliberately gets no `user_id`: it hangs off `recipe_id`, and
-- duplicating the owner onto the child rows would create two sources of truth
-- that can disagree.
ALTER TABLE meal_entries ADD COLUMN user_id INTEGER NOT NULL DEFAULT 1;
ALTER TABLE recipes ADD COLUMN user_id INTEGER NOT NULL DEFAULT 1;

CREATE INDEX IF NOT EXISTS idx_meal_entries_user_date ON meal_entries(user_id, date);
CREATE INDEX IF NOT EXISTS idx_recipes_user ON recipes(user_id);
