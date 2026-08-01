# Redesign plan · /fitness — Fitness tracker

*(Markdown transcription of `Redesign Plan.dc.html` from the design project.)*

A phone-first rebuild of the nutrition tracker in `drawingportfolio`. Same
stack — Rust, Askama, HTMX, one stylesheet — new information architecture,
targets, meal slots and a scan-first logging path.

## 1 · What exists today

The page is a single 640px column rendered by `templates/fitness/feed.html`,
with the day block and the food library built as Rust format strings in
`src/routes/nutrition.rs` (`day_section_html`, `library_list_html`) and swapped
over HTMX. It works, and it is honest about what it is: three ISO-date buttons,
a flat totals strip, an undivided entry list, an inline select-and-grams form,
then an alphabetical food library with an add form and a barcode scanner hidden
behind a secondary button.

Concretely, the friction is:

- Logging a normal meal is select → portion → grams → Log, repeated per food,
  with the food select holding every item ever added.
- Totals have no denominator, so a day reads as four numbers with no verdict.
- Entries are one undifferentiated list — no breakfast/lunch/dinner, and a
  logged entry can only be deleted, never edited.
- Barcode scan — the fastest way to add a packaged food — sits third in a row
  of secondary buttons inside the library section.
- Only one day is ever visible; the week exists in the database and nowhere in
  the UI.
- The visual layer is browser default: system-ui, #ddd borders, 4px radii, no
  type hierarchy below 0.8rem.

## 2 · Principles

- **Thumb, not desk** — Every action reachable one-handed; nothing below a
  44px target. The desktop view is the same column, wider.
- **Two taps to log** — Scan or a recent chip, then one portion button. Typing
  grams is the fallback, not the path.
- **Numbers with a verdict** — Every total is shown against its target:
  remaining calories, macro rails, days hit.
- **Server-rendered still** — HTMX fragments and one stylesheet. No framework,
  no build step, no client state to keep in sync.

## 3 · Screens

| Screen | What changes |
|--------|--------------|
| **Today** (mockup 1a) | Week strip replaces the date input (tap a day, arrows still work). Calorie ring plus three macro rails against targets. Entries grouped into breakfast / lunch / dinner / snack with per-slot totals; empty slots offer their own add row. Sticky bottom bar: Scan · Search · Copy yesterday. |
| **Add sheet** (mockup 1b) | Scanner opens as the default tab, full width, with the existing `BarcodeDetector` / OpenFoodFacts path behind it. Tabs for Recent, Favourites, Meals, Search. A match becomes a card with one-tap portions (package fractions and custom portions, already in the data) and a meal-slot choice. |
| **Food library** (mockup 1c) | Grouped by category with filter chips (Favourites, Recent, High protein, No macros yet). Rows become cards with the thumbnail, macro line and package badge. Search stays client-side over the rendered list. |
| **Food detail** (mockup 1d) | The eight-field nutrient grid becomes a per-100g table you tap to edit, replacing the current two-column form. Package size and custom portions become editable chips. A 14-day log history shows whether the item is actually used. |
| **Week** (mockup 1e) | New route. Seven-day calorie bars against the target line, protein average and hit rate, days logged and streak, weight trend with a one-tap log, most-logged foods. |

## 4 · What the redesign needs from the backend

Four migrations, all additive — nothing existing has to move.

- **008** — `meal_entries.slot TEXT DEFAULT 'other'` and `logged_at`. Existing
  rows land in "other", which renders as an unlabelled group, so no backfill is
  required.
- **009** — a one-row `targets` table (calories, protein, carbs, fat) plus
  `get_targets()` / `set_targets()`. Single-user, so no user id.
- **010** — `food_items.category`, `is_favourite`; a `recent_foods` view over
  `meal_entries` gives the chips and the Recent tab for free.
- **011** — `weights(date, kg)` and `recipes` + `recipe_items` (a saved meal is
  a named list of food_item + grams; logging one inserts its rows).
- New handlers: `PUT /api/nutrition/entries/{id}` (edit a logged entry),
  `POST /fitness/copy-day`, `GET /fitness/week`, `POST /api/nutrition/weights`.

## 5 · Sequence

1. **Visual layer only.** Rewrite the `/* ── Fitness Tracker ── */` section of
   `static/style.css` against the new tokens, and give `base.html` a token
   block. No Rust changes; the page looks new the same day.
2. **Targets + ring.** Migration 009, then `day_section_html` renders
   remaining-against-target instead of bare sums.
3. **Meal slots.** Migration 008, group in the handler, one add row per slot.
4. **Add sheet.** Promote the scanner, add the recents/favourites tabs and
   portion buttons; the log form posts to the same endpoint with a slot field.
5. **Library + detail.** Migration 010, category grouping, edit-in-place
   detail sheet.
6. **Week, weight, saved meals.** Migration 011 and the new route.

Each step ships on its own and touches the layers the project already assigns:
SQL in `db.rs`, fragments in `nutrition.rs`, styles under a named section
comment, and the palette command list updated when the week route lands.

## 6 · Open questions — RESOLVED 2026-08-01

- ~~Should the tracker stay publicly readable?~~ → **Gate behind the session.**
- ~~Fixed daily targets, or training/rest days?~~ → **Fixed daily targets.**
- ~~Meal slots by name, or by clock?~~ → **Clock-inferred default, one tap to change.**
- ~~Does the week view need editing?~~ → **Read-only, taps into a day.**
