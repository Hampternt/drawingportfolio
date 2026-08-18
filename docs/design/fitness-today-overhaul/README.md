# Handoff: Fitness Tracker — "Today" logging overhaul

## Overview

The `/fitness` Today screen in `Hampternt/drawingportfolio` (Rust + Axum + Askama + HTMX,
branch `master`) works but is slow to log with. This handoff covers a redesign of that one
screen that:

1. Moves **quantity onto the logged row** as one-tap fractions of the food's own basis
   (pack / breast / scoop / tbsp), replacing the select → portion-select → grams-field path.
2. Adds **one-tap re-logging** of recent foods and **batch logging** of saved meals.
3. Lets the **search field create a food** that is immediately loggable.
4. Fixes two real bugs (slot picker resets to clock time; a newly added food never reaches
   the log form's `<option>` list).
5. Adds day-level macro composition (pie + legend), per-row macro colour coding, "left to
   hit today", and "usual at \<slot\>" — and a phone layout.

## About the design files

The files in this bundle are **design references created in HTML** — interactive prototypes
of the intended look and behaviour, not production code to copy. The target codebase is
server-rendered Askama templates + one global stylesheet + HTMX, with no build step and no
JS framework. **Recreate these designs there**, in that idiom: Askama partials for the
markup, plain CSS in `static/style.css` (or the new `styles.css` token layer), HTMX
attributes for the interactions, and small vanilla-JS handlers only where noted below.

The prototype's React-ish structure (`renderVals`, state objects) is an artifact of the
prototyping environment. Ignore it; the **State management** section below states the same
logic in server-side terms.

## Fidelity

**High fidelity.** Colours, typography, spacing, radii and interaction states are final and
come from the Hampter Design System tokens (copied into `design-system/` in this bundle —
those files are authoritative if any value here disagrees). Layout dimensions are exact.
Copy is final: use it verbatim.

Two things are explicitly placeholder:
- **Food thumbnails** are letter tiles with a texture-grid background and a coloured ring.
  Real thumbnails come from each item's existing `image_url` (uploads + OpenFoodFacts
  matches). Keep the ring colour and the 34 / 30 / 26 px sizes; swap the letter for the image.
- **Week strip data** beyond today is dummy.

Out of scope / not designed yet: the Add sheet (scanner, manual macro entry), the week view,
the targets editor. The buttons for those exist in the design and should keep their current
behaviour.

---

## Screens / Views

There is one screen, in two layouts. A layout toggle exists in the prototype only (a demo
affordance) — in production, layout follows viewport width.

### 1. Today — desktop (≥ ~900px)

**Purpose:** see where the day stands, then log food in as few taps as possible.

**Layout**
- Page: `background: var(--surface-page)` (`#0B0910`), `color: var(--text-body)`,
  base font `var(--type-body)` (Space Grotesk 400 15px/1.5), `padding-bottom: 64px`.
- Sticky header, 56px tall, `z-index: 30`, `background: rgba(14,12,20,.82)` +
  `backdrop-filter: blur(10px)`, `border-bottom: 1px solid var(--border-subtle)`,
  `padding: 0 24px`, `gap: 24px`. Contents: wordmark `hampter` in Archivo 800 18px/1,
  `letter-spacing: -.035em`, `color: var(--text-strong)`, with the full stop in
  `var(--accent)`; nav links at `var(--type-ui)` and `gap: 20px` (active link
  `color: var(--text-strong)` + `box-shadow: inset 0 -2px 0 var(--accent)`, others
  `var(--text-muted)`); right side: user name at `var(--type-label)` / `var(--text-faint)`
  and a `sm` `secondary` Button with a `command` icon and a `K` shortcut cap, labelled "Search".
- Shell: `max-width: 1180px; margin: 0 auto; padding: 0 24px`.
- Page head: `display:flex; align-items:flex-end; justify-content:space-between; gap:16px;
  flex-wrap:wrap; padding: 32px 0 24px`, with the 32px blueprint grid behind it
  (`background-image: var(--texture-grid); background-size: var(--texture-grid-size)`).
  Left: micro-label `MON 17 AUG · 2026-08-17` (`var(--type-label)`, uppercase,
  `letter-spacing: var(--tracking-caps)`, `var(--text-muted)`) above `<h1>Today</h1>` at
  `var(--type-h1)` (Archivo 800 40px/1.22), `letter-spacing: var(--tracking-tight)`.
  Right: button row, `gap: 8px` — ghost "Yesterday" (`chevron-left`), secondary "Week"
  (`calendar-days`). (The prototype's two extra buttons — layout toggle, notes toggle — are
  demo-only; drop them.)
- Two columns: `display:flex; gap:24px; align-items:flex-start; flex-wrap:wrap`.
  - Left column: `flex: 1 1 320px; max-width: 360px; gap: 16px; position: sticky; top: 80px`.
  - Right column: `flex: 2 1 480px; gap: 16px`.

**Left column, top to bottom**

1. **Summary card** (`Card`, i.e. `background: var(--surface-card)` `#17141F`, 1px
   `var(--border-subtle)` hairline, `border-radius: var(--radius-md)` 8px, no shadow at rest).
   - Row: `CalorieRing` at **132px** + a 12px-gap column of three `MacroRail`s:
     `protein` `var(--status-warning)` `#FFB570`, `carbs` `var(--status-info)` `#7AA2F7`,
     `fat` `var(--status-danger)` `#F7768E`. Values/targets: `2400 kcal`, `165 / 260 / 72 g`.
   - Week strip: 7 buttons, `gap: 4px`, each `flex:1; height:44px`, column layout,
     `border-radius: var(--radius-sm)`, letter at `var(--type-label)` on top and a
     progress bar at the bottom (`height = min(pct,100) × 0.18` px, `border-radius: 2px`).
     Selected: `border-color: var(--border-accent)`, `background: var(--accent-tint)`,
     `color: var(--text-accent)`, bar `var(--accent)`. Past unselected: `background:
     var(--surface-inset)`, `color: var(--text-faint)`, bar `var(--ink-600)`. Future:
     transparent background.
   - Divider (`1px solid var(--border-subtle)`, 14px above/below), then **day macro pie**:
     a 76px circle, `border-radius: 999px`,
     `background: conic-gradient(var(--status-warning) 0 P%, var(--status-info) P% P+C%, var(--status-danger) P+C% 100%)`
     where P/C/F are **shares of calories** (protein ×4, carbs ×4, fat ×9). Beside it, a
     5px-gap legend: micro-label "WHERE THE CALORIES CAME FROM", then three mono rows
     (`var(--type-mono)`), each an 8px × 8px `border-radius: 2px` swatch + `protein 31% · 168 g`.
     Each row's text and swatch use that macro's colour.
   - Divider, then footer row: `1840 / 2400 kcal` in `var(--type-mono)` / `var(--text-muted)`,
     and a `sm` ghost Button "Targets" (`sliders-horizontal`).

2. **"Left to hit today"** card: micro-label heading, then a
   `grid-template-columns: 1fr 1fr; gap: 10px` of four tiles (`kcal`, `protein`, `carbs`,
   `fat`). Tile: `padding: 8px 10px`, `background: var(--surface-inset)`,
   `1px solid var(--border-subtle)`, `border-radius: var(--radius-sm)`; label in
   `var(--type-label)` uppercase in that macro's colour; value in mono 15px
   `var(--text-strong)`. Over target shows `+N` and the value turns `var(--status-danger)`.

3. **"usual at \<slot\>"** card: heading `usual at breakfast` (lowercase slot name) +
   `one tap` at `var(--type-label)` / `var(--text-faint)` on the right. Then up to three
   full-width `md` secondary buttons, `gap: 6px`, each `justify-content: flex-start;
   gap: 10px`: 26px thumbnail, food name, and the last-logged grams at
   `var(--type-label)` / `var(--text-faint)`. One tap logs it. Sets per slot:
   breakfast = skyr / oats / banana, lunch = chicken / rice / broccoli,
   dinner = chicken / rice / olive oil, snack = peanut butter / whey / banana.
   In production, derive this from the user's own most-frequent items for that slot.

4. **Streak** card (`variant="inset"`): `9` in Archivo 800 40px/1 `var(--accent-warm)`
   beside "days logged in a row" at `var(--type-body-sm)` / `var(--text-muted)`.

**Right column, top to bottom**

5. **Log card** — the entry point.
   - Slot row: micro-label "LOG TO", then four `sm` buttons (`breakfast lunch dinner snack`,
     lowercase); the selected one is `primary`, the rest `ghost`. When the selected slot is
     not the clock-inferred one, a note appears: `staying on lunch` at `var(--type-label)` /
     `var(--text-muted)` plus a `sm` ghost button `now: dinner` that snaps back. Far right:
     `3 to check` in `var(--accent-warm)`, or `all amounts checked` in `var(--text-faint)`.
   - Search row (`gap: 8px`, `margin-top: 14px`): a 34px-tall field — `padding: 0 12px`,
     `background: var(--surface-inset)`, `1px solid var(--border-default)`,
     `border-radius: var(--radius-sm)` — containing a 14px `search` icon, the input
     (`var(--type-body-sm)`, `color: var(--text-strong)`, no border/outline, placeholder
     `Type a food, ↵ logs your usual portion`), and a `Kbd` showing `/`. Beside it, an `md`
     secondary Button "Scan" (`scan-line`).
   - **Log again / Matches**: micro-label heading (`Log again` with an empty query,
     `Matches — tap to log` while typing), then up to 6 `md` secondary chips, `gap: 6px`,
     each showing the food name + its last-logged grams in `var(--type-label)` /
     `var(--text-faint)`. Tapping one logs that food at that amount into the selected slot.
   - **Create-and-log**: when the query matches nothing, a `md` **primary** button appears:
     `+ Add “<query>” and log 100 g`. It creates the food (macros empty, flagged
     `needsMacros`) and logs 100 g immediately, in one action.
   - Divider, then **Saved meals**: micro-label + a `sm` ghost "New meal" / "Close" toggle;
     `md` secondary chips per meal showing name + total kcal. Tapping one logs **every item**
     into the selected slot, each row flagged. Seed meals: "Post-gym shake"
     (whey 30 g, banana 120 g, skyr 250 g), "Chicken & rice" (chicken 180 g, rice 250 g,
     olive oil 10 g), "Oats + skyr" (oats 80 g, skyr 125 g).
   - **Meal builder** (collapsed): a panel — `padding: 12px`,
     `background: var(--surface-inset)`, hairline, `radius-sm` — with a "Meal name" input
     (34px, same field styling), a running kcal total in mono `var(--text-muted)`, a `md`
     primary "Save meal", a row of picked items as `Tag`s each with an `✕` remove, and a row
     of `sm` ghost `+ <food>` buttons (first 8 foods) to add items.

6. **Meal slot cards** — one `Card` per slot (`breakfast`, `lunch`, `dinner`, `snack`),
   16px apart.
   - Slot header: `padding-bottom: 10px; border-bottom: 1px solid var(--border-subtle)` —
     lowercase slot name at `var(--type-label)` uppercase-transformed, tracked
     `var(--tracking-caps)`, `var(--text-strong)`; slot kcal in mono `var(--text-faint)`
     (`—` when empty); `+ add` as a `sm` ghost button, right-aligned, which selects that
     slot in the picker.
   - **Logged row** — the core unit. `padding: 12px 0`, `border-bottom: 1px solid
     var(--border-subtle)`. Freshly logged rows additionally get
     `box-shadow: inset 2px 0 0 var(--accent-warm)` + `padding-left: 10px` and a 6px amber
     dot before the thumbnail.
     - Left: 34px thumbnail (`radius-sm`, `background: var(--surface-chip)` with
       `var(--texture-grid)` at `background-size: 8px 8px`, `box-shadow: inset 0 0 0 1px
       <dominance colour>`, letter in Archivo 800 13px/1 uppercase in the same colour).
     - Middle: food name (`var(--type-body)`, `var(--text-strong)`; brand appended as
       `Skyr natural · Arla`), then a 10px-gap wrapping meta row 4px below: a **22px macro
       pie** (same conic-gradient recipe, from that row's own macro grams), `P 27.5 g` in
       `#FFB570`, `C 10 g` in `#7AA2F7`, `F 0.5 g` in `#F7768E`, the dominance label
       (`protein` / `carbs` / `fat` / `balanced` / `no macros`) in the dominance colour, and
       the basis `pack 500 g` in `var(--text-faint)` — all at `var(--type-label)`.
     - Right: a button showing `250 g` in mono `var(--text-body)` and the row kcal in mono
       15px `var(--text-strong)` (`min-width: 56px`, right-aligned). Tapping it expands or
       collapses the amount controls. Then a 28px `✕` remove button,
       `color: var(--text-faint)`, transparent border, `radius-sm`.
     - **Amount controls** (expanded by default on a freshly logged row, otherwise
       collapsed): `margin-top: 10px; padding-left: 46px`. A
       `grid-template-columns: repeat(auto-fit, minmax(76px, 1fr)); gap: 6px; width: 100%`
       grid of two-line buttons — `padding: 5px 10px`, `flex-direction: column`,
       `line-height: 1.2`, top line `var(--type-ui)`, bottom line `var(--type-label)` at
       `opacity: .7`:
       - `full` / `½` / `⅓` / `¼` of the food's **basis** (`base` grams), each showing its
         grams; entries under 3 g and duplicates within 0.5 g are dropped.
       - `last` — the amount last logged for that food, if it isn't already one of the above.
       - `custom` / `g` — toggles a nudge row.
       The button matching the row's current grams (±0.5 g) is `primary`; the rest are
       `secondary`. Active `custom` gets `border-color: var(--border-accent); color:
       var(--text-accent)`.
     - **Nudge row** (when `custom` is on): `−` and `+` `md` ghost buttons with
       `1px solid var(--border-subtle)` and `min-width: 44px`, around a centred 34px
       `number` input in mono. Step is 10 g, or 5 g below 50 g (see `stepGrams`).
   - Empty slot: `> nothing logged` in mono `var(--text-faint)`, `padding: 12px 0`.

7. **Food library** card (`variant="inset"`, collapsed by default): header row with
   micro-label "FOOD LIBRARY", item count in mono `var(--text-faint)`, and a `sm` ghost
   `Open` / `Hide` on the right. Open, it lists foods **grouped by nutrition profile**, not
   category: `mostly protein` · `mostly carbs` · `mostly fat` · `balanced` ·
   `macros missing`. Group heading: `var(--type-label)` uppercase, tracked, in the group
   colour, with the count in mono `var(--text-faint)`; `margin: 12px 0 6px`; hidden when the
   group is empty. Row: `padding: 8px 0`, hairline bottom, `gap: 12px` — 30px thumbnail,
   18px macro pie, name, then `63 kcal / 100 g` (`var(--text-muted)`), `P 11` `#FFB570`,
   `C 4` `#7AA2F7`, `F 0.2` `#F7768E`, the basis in mono `var(--text-faint)`, and a `sm`
   secondary `Log` button.

8. **Toast** (bottom-right, `position: fixed; right: 24px; bottom: 24px; z-index: 60`):
   `padding: 10px 12px`, `background: var(--surface-overlay)`,
   `1px solid var(--border-default)`, `radius-sm`, `box-shadow: var(--shadow-3)`. Text at
   `var(--type-body-sm)`, e.g. `Logged Skyr natural · 250 g` or
   `Logged Post-gym shake · 3 items`, plus a `sm` ghost **Undo** in `var(--text-accent)`.
   Auto-dismisses after 5s; Undo removes the whole batch it refers to.

### 2. Today — phone (≤ ~430px; prototype frame is 390px)

Same content, same components, these differences:
- Single column, `gap: 12px` between cards (16px on desktop).
- Header nav links hidden; wordmark + Ctrl-K button remain.
- Page head `padding: 16px 0 12px`, title drops to `var(--type-h2)` (Archivo 700 26px), and
  the grid texture is dropped.
- `CalorieRing` 104px; day pie 64px.
- The left column's cards stack above the log card in the same order.
- **Sticky bottom action bar**: `position: sticky; bottom: 0; z-index: 20`, `gap: 8px`,
  `padding: 12px 16px 20px`, bleeding to the frame edges (`margin: 12px -16px -24px`), over
  `linear-gradient(to top, var(--surface-page) 70%, rgba(11,9,16,0))`. Contents: `lg`
  primary block "Scan" (`scan-line`), `lg` secondary block "Search" (`search`), and an
  icon-only `lg` secondary `copy` button (copy yesterday).

---

## Interactions & behaviour

Every one of these is a small, local change — they suit HTMX fragment swaps.

| Action | Result |
| --- | --- |
| Tap a slot in the picker | Slot becomes selected and **stays** selected. The clock only seeds it on first load. |
| Tap `now: <slot>` | Selected slot snaps back to the clock-inferred one. |
| Tap a "Log again" chip / a "usual at" button / a library `Log` | Logs that food into the selected slot at its **last-logged** grams (falling back to its `usual`), flags the row fresh, expands its amount controls, clears the query, shows the toast. |
| Type a query | Filters the food list to 6 matches; heading changes to `Matches — tap to log`. |
| Query matches nothing | Primary `+ Add “x” and log 100 g` appears; tapping it creates the food and logs 100 g in one request. |
| Tap a saved meal | Logs all its items into the selected slot, all flagged, one toast covering the batch. |
| Tap a row's grams/kcal | Toggles that row's amount controls; clears its fresh flag. |
| Tap a fraction / `last` button | Sets the row's grams, clears the fresh flag, and **records that amount as the food's last-logged value**. |
| Tap `custom` | Reveals the `− [input] +` nudge row for that row. |
| `−` / `+` | ±10 g (±5 g below 50 g), floor 1 g, rounded to 0.1 g. |
| Tap `✕` on a row | Removes the entry. |
| Tap `+ add` in a slot header | Selects that slot in the picker. |
| Tap `New meal` | Opens the builder; `+ <food>` adds it at its usual grams; `Save meal` stores it and shows a toast. |
| Tap `Undo` in the toast | Removes the entries from the batch the toast refers to. |
| Toast | Dismisses itself after 5000 ms. |

**Motion:** controls 130ms, surfaces 190ms, overlays 280ms, all
`cubic-bezier(.2,.8,.3,1)`. Rails and the ring animate their fill over 280ms. Press state is
`translateY(1px)`, never a scale. Focus is the system's double ring
(`0 0 0 2px var(--surface-page), 0 0 0 4px var(--violet-400)`). Honour
`prefers-reduced-motion` (the token layer already zeroes durations).

**Keyboard:** `/` focuses the search field (the design shows the `Kbd` hint), `Ctrl`/`⌘` `K`
opens the palette, `Esc` closes/blurs. Do not rebind reserved keys.

**Responsive:** single breakpoint, around 900px, switching two columns → one and revealing
the sticky bottom bar. The row's amount-button grid is already fluid
(`auto-fit, minmax(76px, 1fr)`), so it reflows without a breakpoint.

**Hit targets:** amount buttons, nudge buttons and slot chips are ≥ 34px tall; nudge buttons
are `min-width: 44px`. Do not shrink these on phone.

---

## State management

Server-side state per user per day:

- `entries[]` — `{ id, food_id, grams, slot, fresh }`. `fresh` is session-scoped: set on
  insert, cleared when the user touches that row's amount or expands it. It drives the amber
  edge, the dot, and the `N to check` count. It does not need to persist across sessions.
- `selected_slot` — **the fix.** Currently `initSlotDefault()` re-runs on every
  `htmx:afterSwap` and `day_section_html` re-renders `slot=other`, so each log snaps the
  picker back to the time-of-day slot. Echo the submitted slot back in the day fragment and
  let the clock seed it only on the initial page render.
- `expanded` / `custom` — per-row UI flags. Client-side is fine; a fresh row starts expanded.
- `last_grams[food_id]` — the amount last logged for each food. Feeds the `last` button, the
  "Log again" chip labels and the "usual at" grams. Persist this; it is what makes one-tap
  logging accurate.
- `foods[]` — **one catalog** feeding search, the chips, the library and the meal builder.
  Today the log form's `<option>` list is baked into `day_section_html` while adding a food
  only swaps `#food-library`, which is why a new food never appears in the dropdown. With
  quantity living on the row, the dropdown goes away entirely — but the single-source rule
  still matters for the chips and the builder. A food created from search has
  `needs_macros = true`, renders as `no macros yet` / `macros missing`, and should prompt for
  macros later.
- `meals[]` — `{ id, name, items: [{ food_id, grams }] }`. Creatable from the builder, not
  only from an already-logged slot.

Derived values (compute server-side, they are all cheap):
- Row kcal and macros: `food.<field> × grams / 100`.
- Day totals; `left = target − total` (negative renders as `+N` in `--status-danger`).
- Calorie shares for the pies: protein ×4, carbs ×4, fat ×9, normalised to 100%.
- Dominance: the largest share; under 45% ⇒ `balanced`; `needs_macros` ⇒ `unknown`.
- Fraction buttons: `base × {1, ½, ⅓, ¼}`, rounded to whole grams at ≥20 g else 0.1 g,
  dropping anything under 3 g and any duplicate within 0.5 g.
- Basis (`base`, `base_name`): the pack the food comes in, else the unit it is served in
  (breast, scoop, tbsp, "2 eggs"). This needs a **schema addition** per food — it is the one
  new field the design depends on. Fall back to `usual`, then 100 g.
- Clock seeding thresholds (unchanged from `defaultSlot()`): `<11` breakfast, `<15` lunch,
  `<17` snack, `<22` dinner, else snack.

**Fragment boundaries** worth keeping (each returns just what changed, swapped `outerHTML`):
the summary card, one slot card, one logged row, the library, the toast.

**HTMX caveat:** any JS initialiser must bind on both `DOMContentLoaded` and
`htmx:afterSwap`, guard `e.target === document.body`, and not stack duplicates.

---

## Design tokens

Authoritative copies are in `design-system/` in this bundle. The values the design uses:

**Colour**
| Token | Value | Used for |
| --- | --- | --- |
| `--surface-page` / `--ink-950` | `#0B0910` | page |
| `--surface-card` / `--ink-850` | `#17141F` | cards |
| `--surface-inset` / `--ink-900` | `#0E0C14` | inputs, inset tiles, note blocks |
| `--surface-chip` / `--ink-700` | `#262232` | thumbnails |
| `--surface-overlay` | `#17141F` | toast |
| `--text-strong` / `--ink-050` | `#F2EEF8` | headings, values |
| `--text-body` / `--ink-100` | `#CDC6DD` | body |
| `--text-muted` / `--ink-300` | `#8D87A0` | labels, secondary |
| `--text-faint` / `--ink-400` | `#5F5876` | metadata |
| `--accent` / `--violet-400` | `#B48EF7` | primary button, active tab/underline, focus, wordmark stop |
| `--text-accent` / `--violet-300` | `#CBB0FF` | accented text, Undo |
| `--accent-tint` | `rgba(180,142,247,.14)` | selected week day |
| `--accent-warm` / `--amber-400` | `#FFB570` | streak number, fresh-row edge and dot, `N to check` |
| `--status-warning` | `#FFB570` | **protein** |
| `--status-info` / `--azure-400` | `#7AA2F7` | **carbs** |
| `--status-danger` / `--rose-400` | `#F7768E` | **fat**, over-target values |
| `--border-subtle` | `rgba(242,238,248,.07)` | hairlines |
| `--border-default` / `--ink-700` | `#262232` | input borders |
| `--border-accent` | `rgba(180,142,247,.45)` | active custom, selected week day |
| `--ink-600` | `#322C42` | inactive week bars |

Note: protein and `--accent-warm` are the same amber. That is intentional — the fresh-row
flag and the protein rail never sit on the same element.

**Type** — Archivo (self-hosted, 800/900) for display, Space Grotesk (self-hosted, 400/500)
for UI and body, IBM Plex Mono for anything machine-produced. Roles used here:
`--type-h1` 800 40px/1.22 Archivo · `--type-h2` 700 26px/1.22 · `--type-body` 400 15px/1.5
Space Grotesk · `--type-body-sm` 400 13px/1.5 · `--type-ui` 500 14px/1.2 ·
`--type-mono` 400 13px/1.45 · `--type-label` 500 11px/1.2 mono (uppercase micro-labels add
`letter-spacing: .10em`). Tracking: `--tracking-tight -0.025em` on display,
`--tracking-caps 0.10em` on micro-labels. Sans is never uppercased; mono is never used for
paragraphs.

**Spacing** — 4px-derived with fine low steps: 2, 4, 6, 8, 10, 12, 14, 16, 20, 24, 32, 40,
48, 64. Control heights: 28 (`sm`), 34 (`md`), 42 (`lg`).

**Radii** — `--radius-xs 3px` (keycaps, badges) · `--radius-sm 5px` (buttons, inputs, tiles,
thumbnails) · `--radius-md 8px` (cards) · `--radius-lg 12px` (dialogs) · `--radius-xl 18px`
(the phone frame) · `999px` (pies, dots, tags).

**Shadows / effects** — `--shadow-3 0 18px 48px rgba(0,0,0,.6)` (toast only);
`--focus-ring 0 0 0 2px var(--surface-page), 0 0 0 4px var(--violet-400)`;
`--texture-grid` (32px blueprint grid, 4.5% white) behind the page head, and the same
gradient pair at `8px 8px` inside thumbnails. Cards carry no shadow at rest.

---

## Assets

- **Icons:** Lucide v0.462.0, rendered as a CSS mask over `currentColor`. Used here:
  `command`, `search`, `scan-line`, `calendar-days`, `chevron-left`,
  `sliders-horizontal`, `copy`. This is a flagged substitution — the repo ships no icon set;
  swap freely, only the `Icon` wrapper changes.
- **Fonts:** `assets/fonts/*.woff2` in the design system are the repo's own files (Archivo
  500–900, Space Grotesk 400–700); serve them from `/static/fonts/`. IBM Plex Mono is the one
  external face (Google Fonts) — self-host it if you'd rather not pay the DNS lookup.
- **Food thumbnails:** placeholders. Real images come from each item's `image_url`.
- No photography, illustration or gradient mesh anywhere.

## Files in this bundle

| File | What |
| --- | --- |
| `Fitness Today (overhaul).dc.html` | The redesign. Open it in a browser; it is fully interactive (log, adjust, undo, build a meal, toggle the phone layout). The violet numbered notes 01–05 explain each change and the two bugs. |
| `Fitness Today (current).dc.html` | The **existing** `/fitness` screen recreated from source, for before/after comparison. |
| `support.js` | Runtime the two HTML files need. Not part of the deliverable. |
| `design-system/` | Authoritative token CSS + component CSS from the Hampter Design System (`tokens/*.css`, `components/components.css`, `styles.css`). |
| `github.md` | Repo association and the screen → source-file map for the current screen. |

Open the two HTML files side by side before starting; the interaction rhythm of the row
amount buttons is the point of the redesign and is easier to feel than to read.
