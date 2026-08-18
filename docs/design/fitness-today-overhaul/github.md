repo: Hampternt/drawingportfolio
branch: master
path: (fitness tracker: templates/fitness, src/routes/nutrition.rs, static/style.css)

## Last sync

date: 2026-08-17T01:15:00Z

### Updated in this project

- Recreated the current `/fitness` Today screen and Add sheet from source, pixel-faithful.
- Read the Nocturne token block and every `.noc-*` / fitness rule in `static/style.css`.
- Read every fragment builder in `src/routes/nutrition.rs` (ring, rails, week strip, slots, log form, match card, meals pane, library).

## Screen map

| Project screen | Repo files |
| --- | --- |
| Fitness Today (current).dc.html — Today frame | templates/fitness/feed.html, templates/base.html, src/routes/nutrition.rs (`day_section_html`, `week_strip_html`, `calorie_ring_svg`, `macro_rail_html`, `library_list_html`, `food_item_card_html`, `meal_entry_row_html`), static/style.css (Nocturne tokens + Fitness Tracker sections) |
| Fitness Today (current).dc.html — Add sheet frame | templates/fitness/feed.html (`#add-sheet`), src/routes/nutrition.rs (`match_card_html`, `recent_chips`, `meals_pane_html`, `food_search`), static/style.css (add sheet section) |
