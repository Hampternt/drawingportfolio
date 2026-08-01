ALTER TABLE food_items ADD COLUMN category TEXT NOT NULL DEFAULT '';
ALTER TABLE food_items ADD COLUMN is_favourite INTEGER NOT NULL DEFAULT 0;
ALTER TABLE food_items ADD COLUMN default_portion_g REAL;
