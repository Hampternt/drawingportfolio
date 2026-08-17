# Container: Fitness "Today" overhaul

**Status:** Packs 1–3 landed 2026-08-17 (gates green, walkthroughs below). Pack 4 next.
**Where:** `~/projects/drawingportfolio.worktrees/fitness-today-overhaul` on
`feat/fitness-today-overhaul`, branched from `dev` @ 49c56cd (multi-user fitness
landed; `dev` and `master` are level).
**Spec:** `docs/design/fitness-today-overhaul/` — the README is the spec; the
`design-system/tokens/*.css` files are authoritative where they disagree with it.
**Scope:** the `/fitness` Today screen only. The Add sheet, the week view and the
targets editor keep their current behaviour.

## Goal

Make logging food fast. Today's screen works but every log costs a
select → portion-select → grams-field round trip. The redesign moves quantity
**onto the logged row** as one-tap fractions of the food's own basis, adds
one-tap re-logging and batch meal logging, lets the search field create a food
that is immediately loggable, and adds the day-level macro composition the
screen currently has no way to show.

Two real bugs die with it:

- **Slot picker resets to clock time.** `feed.html:376` re-runs `initSlotDefault()`
  on every `htmx:afterSwap`, and `day_section_html` re-renders
  `<input name="slot" value="other">` (`nutrition.rs:471`), so every log snaps the
  picker back to the time-of-day slot. Fixed in Pack 2 by echoing the submitted
  slot back in the day fragment and letting the clock seed it on first render only.
- **A newly added food never reaches the log form's `<option>` list.** The list is
  baked into `day_section_html` (`nutrition.rs:390-420`) while adding a food only
  swaps `#food-library`. Fixed in Pack 3 **by deletion** — with quantity on the row
  the dropdown goes away entirely. Do not patch it in the meantime.

## Pack sequence

| # | Pack | Ends in something you can see |
| --- | --- | --- |
| 1 | **Token layer + page shell** | `/fitness` wears the new palette, type and two-column frame |
| 2 | **The logged row** | Log something, then set its amount from the row itself |
| 3 | **One-tap logging** | Re-log, create-and-log, and batch a saved meal in one tap |
| 4 | **Day at a glance** | Summary card, macro pie + legend, "left to hit today", regrouped library |
| 5 | **Phone layout + polish** | The phone layout with its sticky action bar; motion and keyboard |

Only Pack 1 is planned to item level. Later packs get their manifests when they start.

<details>
<summary><b>Pack 1 — Token layer + page shell</b> (done 2026-08-17)</summary>

**Done when:** `/fitness` renders on the Hampter Design System tokens, self-hosted
type, and the overhaul's desktop frame — sticky header, page head, two columns —
with the existing cards sitting inside it, and no other page changes appearance.

- [x] **1.1 Token layer, scoped.** Paste the `:root` blocks from `tokens/colors.css`,
      `typography.css`, `spacing.css`, `shape.css` and `motion.css` into
      `static/style.css` under one named section. Port `tokens/base.css`'s **element**
      rules scoped under `body.fitness-dark` — upstream they are unscoped and would
      restyle `/`, `/artportfolio`, `/tasks` and `/admin`. The rules needing scoping
      are `body{}`, `h1,h2,h3,h4{}`, `p`, `a`, `hr`, `::selection`, `:focus-visible`
      and `button,input,select,textarea{font:inherit}`. Its `*{box-sizing:border-box}`
      is the one exception — `style.css:1` already sets that globally, so it is a
      duplicate, not a change; drop it rather than scope it.
      *Done: `/fitness` resolves every token; the other four pages render unchanged.*
      ⚠ **Flagged for individual review** — cross-page blast radius.
- [x] **1.2 Type wired; Inter dropped.** **No font files to copy** — the roles this
      design actually uses are Archivo 700/800, Space Grotesk 400/500 and IBM Plex
      Mono 400/500, and all seven are already in `static/fonts/`. (`--type-h3`
      wants Archivo 600, which is absent, but the overhaul markup contains no `<h3>`
      or `<h4>`; leave the role undeclared rather than ship an unused face.) So:
      keep `style.css`'s existing `@font-face` block, and **do not** copy the
      `@import url(fonts.googleapis.com…)` line at the top of `tokens/fonts.css` —
      that line is the only Google dependency; `--font-mono` in `typography.css`
      already names `"IBM Plex Mono"` and needs no repointing. Then delete the Inter
      link at `feed.html:7-9`.
      *Done: `/fitness` issues no external font request.*
- [x] **1.3 Nocturne bridge.** Add the `legacy-nocturne.css` mapping block so every
      `--noc-*` resolves to a new token.
      *Done: `/fitness`, `/fitness/week` and the Add sheet repaint with zero markup
      changes.* See **Accepted trade-offs** — this repaints two out-of-scope surfaces.
- [x] **1.4 Page shell.** Sticky 56px header (wordmark with the violet stop, nav,
      user name, `Search` + `K` cap), 1180px shell, page head (`MON 17 AUG · 2026-08-17`
      micro-label, `Today` h1, Yesterday/Week buttons, 32px grid texture), and the
      two-column flex with the left rail `position: sticky; top: 80px`.
      *Done: the desktop frame matches the overhaul prototype; existing cards sit in
      the right column.* Drops the prototype's demo-only layout and notes toggles.

**Item gate:** `./scripts/check.sh`, plus `cargo test --test static_assets` after
any CSS paste — that guard exists for exactly this (a nested `/* */` in a vendor
comment block silently drops the next rule).
**Pack gate:** `./scripts/verify.sh` + a real-browser walkthrough of `/fitness`,
`/fitness/week`, the Add sheet, and one non-fitness page to prove no bleed.

</details>

<details>
<summary><b>Pack 2 — The logged row</b> (done 2026-08-17)</summary>

**Done when:** you log a food and set its amount from the row itself — one tap for
a fraction of its pack, `last`, or a nudge — without the slot picker snapping back
under you.

- [x] **2.1 Basis and amount maths.** Migration 022 adds
      `food_items.base_name TEXT NOT NULL DEFAULT ''` — the one field the design
      needs and the catalog lacks. `MealEntryWithFood` gains `brand`, `image_url`,
      `base_grams`, `base_name` and `last_grams`; last-grams comes from a new
      `get_last_grams_map()` (one query per render, not one per row). Pure
      functions ported from the prototype and tested against its exact rules:
      `macro_shares` (×4/×4/×9, zero-guarded), `dominance` (<45% ⇒ balanced),
      `basis` (`package_size` → `default_portion_g` → 100 g), `amount_options`
      (drop <3 g, dedupe within 0.5 g, whole grams ≥20 g else 0.1) and
      `nudge_step` (5 g under 50 g, else 10 g).
      *Done: tests green; nothing visual changes yet.*
      ⚠ Schema ritual: migrate the dev DB, `cargo sqlx prepare`, commit `.sqlx/`.
- [x] **2.2 The row renders.** Rewrite `meal_entry_row_html` to the full anatomy —
      34px thumbnail (real `image_url` where there is one, else the letter tile with
      its dominance ring), name with brand appended, and the meta row: 22px macro
      pie, `P`/`C`/`F` in their macro colours, dominance label, basis. Slot headers
      get their kcal, `+ add`, and the `> nothing logged` empty state.
      *Done: `/fitness` shows the designed rows — still inert.*
- [x] **2.3 Amount controls.** The `auto-fit, minmax(76px, 1fr)` grid of two-line
      buttons, the `custom` nudge row, and a route that sets one entry's grams and
      returns **just that row**. Selected button = within 0.5 g of current.
      *Done: tap a row's grams, tap ½, the row updates in place.*
- [x] **2.4 Fresh flag.** A newly logged row carries the amber inset edge and dot
      and opens its controls; touching the amount clears it. Client-side, per the
      ruling — the insert returns the new row id.
      *Done: log a food and its row is flagged and open.* (The `N to check` counter
      belongs to the Log card, so it lands in Pack 3.)
- [x] **2.5 Slot sticks; fragments narrow.** The picker keeps the slot you chose —
      the clock seeds it on first render only. Row mutations stop swapping
      `#day-section` wholesale, which **breaks the week-strip refresh** at
      `feed.html:380` (it keys on `e.target.id === 'day-section'`); re-point it.
      *Done: log twice into lunch without the picker snapping back, and the week
      strip still tracks.*

**Item gate:** `./scripts/check.sh` + `cargo test --bin drawingportfolio nutrition::`
— this pack is nearly all logic, so the targeted suite runs every time.
**Pack gate:** `./scripts/verify.sh` + a browser walkthrough of the log → adjust →
remove cycle.

**Gate: green.** `verify.sh` clean; 846 workspace tests (830 → 833 → 846); clippy
**18** from a clean build, down from 19 — `MealEntryWithFood` gained read fields,
collapsing a dead-code warning. CLAUDE.md updated with both.

**2.2 and 2.3 landed in one commit.** The row's markup and its amount grid are one
control: committing the row alone would have shipped a grams button whose
`onclick` called a function that did not exist yet.

<details>
<summary><b>Walkthrough evidence</b> — driven through the running app</summary>

Seeded two entries against the local dev DB: 250 g of a 500 g pack of skyr
(macros, brand, named basis) and 30 g of a macro-less, package-less "Mystery
powder", to exercise both the full and the empty case.

*The row.* Skyr renders `Skyr natural · Arla`, dominance **protein** in
`--status-warning`, `P 27.5 g / C 10 g / F 0.5 g`, basis `pack 500 g`, 158 kcal,
and a pie whose first arc is 71.2% — protein's exact share of 154.5 kcal. Its
amount grid is `full 500 / ½ 250* / ⅓ 167 / ¼ 125 / custom`, ½ marked current.

*The empty case renders as empty, not as zero.* Mystery powder shows a hollow
ring rather than a pie, **no** P/C/F figures, `no macros` in `--text-faint`, and
**no** basis chip — its unnamed basis would otherwise print a bare "100 g" next
to the row's own amount. Its grid falls back to 100 g and gains `last 30 g*`.

*The amount route.* `PUT …/entries/1/grams` with 125 g returns **only** the
`<li>` (verified: response starts `<li`), now reading 125 g, 79 kcal, P 13.8 g,
with `¼` selected. Guards: `0`, `-5`, `99999`, `abc` and a missing field all 400;
an unknown entry id 404s.

*The slot bug, reproduced and fixed.* With the clock reading **snack**, picking
**lunch** and logging leaves the picker on lunch, the chip painted, and the entry
in `slot-lunch`. Before the fix the clock overwrote the choice on every swap.

*The fragment boundary.* Tapping a fraction swaps the row alone
(`swapTargetWasTheRowNotTheDay: true`) and still fires exactly one week-strip
refresh — the breakage this pack predicted, headed off.

</details>

<details>
<summary><b>Deviations and debt</b></summary>

- **The slot fix is client-side, not the server echo the spec describes.** The
  spec suggests echoing the submitted slot back in the day fragment. A
  page-scoped `window.currentSlot` produces the same observable behaviour — the
  clock seeds once and never corrects again — without threading a slot parameter
  through ten handlers that Pack 3 is about to rewrite. Neither version survives
  a full page load, so they are equivalent in reach.
- **`basis()` was written twice and is now written once.** The first cut had the
  fallback chain in both `nutrition.rs` and the day query; clippy caught the Rust
  copy going unused, which is the cheap symptom of the real problem — a basis
  that could disagree between the button label and the logged amount. It now
  lives in `models::basis_grams`, called by both.
- **`N to check` is not built.** It belongs to the Log card, which is Pack 3.
  The fresh flag it counts is in place.
- **Older row CSS is still in the stylesheet.** `.meal-entry`, `.entry-main` and
  friends are now dead for the Today screen but still serve the entry-edit
  fragment. They come out when Pack 3 removes that flow.

</details>

<details>
<summary><b>Review wave — 3 findings, all fixed</b> (<code>high</code>, 2026-08-17)</summary>

Fixed in `ee8dc21`; gate re-run green.

1. **Every amount grid and nudge row was permanently expanded.** Both carry
   `hidden`, but giving them a `display` set an author rule at (0,2,1) against
   the UA's `[hidden] { display: none }` at (0,1,0). The author rule won, the
   attribute stopped meaning anything, and `toggleAmounts`/`toggleCustom`
   flipped state with no visual effect — the pack's central interaction did not
   exist. Restated at author level. This file already documents the same trap
   for `.art-pop`; **any collapsible that gains a `display` needs its
   `[hidden]` rule in the same breath.**
2. **`+ add` did not persist its slot.** It set the form field but not
   `window.currentSlot`, so the choice reverted to the clock on the next swap —
   the picker-reset bug of item 2.5, reached through the slot header instead of
   the picker. One entry point was fixed and its twin was missed.
3. **Nudging an emptied field blanked it.** `parseFloat('')` is NaN, NaN
   survives the arithmetic and `Math.max`, and a number input discards it — so
   the box emptied and the request that followed was rejected with no feedback.
   It now falls back to the row's own amount, which is what the previously
   unread `data-grams` attribute was for.

**Why finding 1 got past the pack walkthrough.** The walkthrough asserted that
a *freshly logged* row's grid was open — and it was, because `flagFreshRow`
removes the attribute. It never asserted that a *non-fresh* row was closed. The
check could not have failed, so it passed while the feature was wholly broken.
A collapse is only verified by measuring both states; the assertion now reads
`display:none` / height 0 at rest and `display:grid` / height 42 once `hidden`
comes off.

</details>

<details>
<summary><b>Pack 3 — One-tap logging</b> (done 2026-08-17)</summary>

**Done when:** logging a familiar food is one tap — from a recent chip, from
"usual at this slot", or a whole saved meal at once — and anything you log can be
undone from the toast that says you logged it.

- [x] **3.1 The Log card.** Replace the log form with the designed card: a
      `LOG TO` slot row (selected slot `primary`, the rest `ghost`), the
      `staying on lunch` / `now: dinner` snap-back when the choice differs from
      the clock, the `N to check` count of fresh rows, and the search field with
      its `/` keycap and Scan button. **Deletes the `<option>` dropdown, and
      with it bug 2** — a new food is no longer absent from a baked-in list
      because there is no list.
      *Done: the log card matches the design and the dropdown is gone.*
- [x] **3.2 Toast and Undo.** A logging action reports the entry ids it created;
      the toast names what landed and Undo removes exactly that batch. 5s
      auto-dismiss. Batch ids ride the existing `HX-Trigger` channel — no schema.
      *Done: log a food, the toast says so, Undo takes it back.*
- [x] **3.3 Log again / Matches, and create-and-log.** With an empty query, up to
      six recents at their last-logged grams; while typing, matches. Tapping one
      logs it into the selected slot. When nothing matches, a primary
      `+ Add "<query>" and log 100 g` that creates the food and logs it in one
      request, flagged `no macros yet` for later.
      *Done: type nothing and re-log in one tap; type junk and create-and-log.*
- [x] **3.4 usual at &lt;slot&gt;.** A left-rail card offering this user's three
      most-*frequent* foods for the selected slot — a different question from
      `get_recent_foods`'s most-*recent*, so it needs its own query.
      *Done: the card tracks the slot picker and one tap logs.*
- [x] **3.5 Saved meals and the builder.** Meal chips logging every item into the
      selected slot as one undoable batch, and a builder that composes a meal
      from any foods rather than only from an already-logged slot.
      *Done: one tap logs a three-item meal; a new meal can be built and saved.*

**Item gate:** `./scripts/check.sh` + `cargo test --bin drawingportfolio nutrition::`.
**Pack gate:** `./scripts/verify.sh` + a walkthrough of every logging path,
including Undo on a multi-item batch.

**Gate: green.** `verify.sh` clean; 848 workspace tests (846 → 848); clippy
holds at **18** from a clean build. CLAUDE.md updated.

**Bug 2 is dead, by deletion as planned.** The `<select>`, its portion select,
the grams field, `onFoodSelect`, `onPortionChange` and the catalog query that
fed them are gone. There is no baked-in list to go stale, so a food created a
moment ago is loggable a moment later.

<details>
<summary><b>Walkthrough evidence</b></summary>

*Chips.* Empty query → `Log again` with recents at their last-logged grams;
`skyr` → `Matches — tap to log`; `zzzznope` → `Nothing matches` plus
`+ Add "zzzznope" and log 100 g`.

*Create-and-log.* One request creates the food and logs 100 g; the row renders
`no macros` rather than pretending it is calorie-free; toast and fresh flag
fire; `1 to check` appears; Undo removes it and a second tap is inert.

*usual at &lt;slot&gt;.* Three breakfast logs of one food and one of another put
the frequent one first — frequency beating recency is the whole reason it is
its own query. An unused slot says "Nothing logged at dinner yet." Tapping the
breakfast chip repoints the card, marks the chip primary, and shows
`staying on breakfast` with `now: snack`.

*Meals.* Adding the same food twice yields one tag; saving produces a chip;
tapping it logs two rows as one batch, toast `Logged Walkthrough shake · 2
items`, both flagged, and Undo removes exactly both.

</details>

<details>
<summary><b>A bug the tests could not have caught</b></summary>

The toast rendered `Logged 12" test pizza Â· 100 g` in the browser while every
unit test passed. **HTTP header values are read as Latin-1**, so the UTF-8
middle dot in the `HX-Trigger` payload arrived double-encoded; an accented food
name would have fared worse. `json_escape` now emits `\uXXXX` for anything
non-ASCII, surrogate pairs included.

The uncomfortable part: my original test asserted
`json_escape("Æbleskiver · Ålborg") == "Æbleskiver · Ålborg"` — it encoded the
broken behaviour as the expectation. Only reading the rendered toast found it.
The test now asserts the header is pure ASCII and that
`12" Æbleskiver 🍕 · 100 g` round-trips exactly, checked in the browser.

</details>

<details>
<summary><b>Deviations and debt</b></summary>

- **The Scan button opens the existing add sheet.** The sheet is out of scope
  per the spec, so the button keeps its current behaviour rather than gaining a
  new one.
- **The builder offers the first eight foods**, as the design shows, with no
  search of its own. Fine while a catalog is small; it will want the search
  field when it is not.
- **`/fitness/htmx/food-search` and the old quick-add still exist**, feeding the
  add sheet and the (now redundant) desktop quick-add strip above the day. Pack
  4 or 5 should remove the strip — the log card replaces it.
- **The entry-edit fragment is now unreachable** from the Today screen: the row
  no longer links to it. Its route, `entry_edit_row_html`, and the `.meal-entry`
  CSS are dead weight to be removed once nothing else references them.

</details>

<details>
<summary><b>Review wave — 2 findings, both fixed</b> (<code>high</code>, 2026-08-17)</summary>

Gate re-run green after the fixes.

1. **`/fitness` hammered the server while nobody touched it.**
   `paintSlotChips` dispatched `slot-changed` unconditionally;
   `#fit-usual-slot` listens for it, fetches, and its response swap re-entered
   the same body-level `htmx:afterSwap` handler that calls `paintSlotChips`.
   The element's `load` trigger started the cycle, so no user action was
   needed. **An idle page issued 1384 requests in three seconds** — about
   460/s at a single-threaded SQLite server, from one tab. Fixed by firing the
   event only on a real change, seeding `lastUsualSlot` from the clock so the
   first paint stays quiet. Now: 0 requests idle, exactly 1 per slot change,
   0 from repeated no-op repaints.
2. **The meal builder kept invisible items.** Its panel lives inside the day
   fragment, so any log destroyed it while `window.mealItems` survived —
   reopening showed an empty list with items still staged, and Save would have
   written foods the user could not see. The panel is now rebuilt from JS state
   after every `#fit-meals` swap, so what is shown and what would be saved
   cannot disagree.

**The lesson worth keeping.** Every walkthrough in this container so far *acted*
and then measured the result, which means an idle page was never once the thing
under test — and an idle page was where the worst bug in the container lived.
The loop surfaced only from reading the event wiring and asking what happens
when nobody does anything. **Watch the network on a page at rest** before
calling an HTMX pack done; a trigger that fires on a swap, and produces a swap,
is the shape to look for.

**Accepted cost, not fixed:** `#fit-meals` and `#log-options` both carry
`hx-trigger="load"` inside the day fragment, so every log spends two extra round
trips re-fetching sections that did not change. The clean fix is lifting the
whole log card out of the day fragment — it renders no day data — which is a
restructure for Pack 4 rather than a review fix.

</details>

<details>
<summary><b>Pack 4 — Day at a glance</b> (in progress)</summary>

**Done when:** the left rail answers "where does the day stand" without
arithmetic — ring, rails, week strip, what the calories were made of, what is
left, and the streak — and the library is grouped by what foods actually are.

- [ ] **4.1 Fragment boundaries, redrawn.** The log card renders no day data yet
      lives inside the day fragment, which is why Pack 3 pays two round trips per
      log and why the meal builder needed rebuilding after every swap. Lift it
      out; `#day-section` becomes the meal slots alone, and the left rail's
      summary refreshes by out-of-band swap on the same response.
      *Done: logging updates slots and summary in one request; an idle page and
      a logging page both show no redundant fetches.*
- [ ] **4.2 Summary card.** 132px calorie ring beside the three macro rails in
      their macro colours, the week strip moved inside the card, and a footer of
      `1840 / 2400 kcal` in mono with the `Targets` button.
      *Done: the card matches the design and tracks the day.*
- [ ] **4.3 Where the calories came from.** The 76px day pie plus its legend —
      three mono rows, each a swatch and `protein 31% · 168 g` in that macro's
      colour. Shares are of **calories**, not grams, which is the whole point.
      *Done: the legend's percentages sum to 100 and match the pie.*
- [ ] **4.4 Left to hit today, and the streak.** Four tiles (kcal, protein,
      carbs, fat) counting down, going `+N` in `--status-danger` once over. The
      streak card reuses `compute_streak`, already written and tested for the
      week page.
      *Done: tiles count down and flip sign; the streak matches the week view.*
- [ ] **4.5 Library by nutrition profile.** Regroup the food library under
      `mostly protein` / `mostly carbs` / `mostly fat` / `balanced` /
      `macros missing` — the same `dominance` the rows already use — collapsed
      by default, each row one tap from being logged.
      *Done: the library groups by what foods are, not by a category field.*

**Item gate:** `./scripts/check.sh` + `cargo test --bin drawingportfolio nutrition::`.
**Pack gate:** `./scripts/verify.sh`, a walkthrough of the day-at-a-glance
numbers against the rows that produce them, **and a network check on an idle
page** — the Pack 3 lesson.

</details>

<details>
<summary><b>Pack 5 — scope only</b></summary>

**Pack 4 — Day at a glance.** Summary card (132px ring, three macro rails, week
strip, day macro pie + "where the calories came from" legend), the "left to hit
today" tile grid, the streak card, and the food library regrouped by nutrition
profile rather than category.

**Pack 5 — Phone layout + polish.** Single column at ≤900px, sticky bottom action
bar (Scan / Search / copy-yesterday), phone type scale, `/` and `Esc` keyboard
handling, motion durations and `prefers-reduced-motion`, hit-target audit.

</details>

## Data model — what the spec asks for vs. what already exists

The README asks for four pieces of server state. Three of them already exist; the
audit matters because two of the obvious implementations would be wrong.

| Spec asks for | Reality | Verdict |
| --- | --- | --- |
| `last_grams[food_id]`, persisted | `get_recent_foods` (`db.rs:1094`) already returns `last_grams` per food, **user-scoped**, from `meal_entries` history | **No column.** The log *is* the persistence. Pack 2 adds a narrowed per-food lookup. |
| the food's `usual` | `user_food_prefs.default_portion_g`, per user since migration 020 | Exists |
| basis grams (`base`) | `food_items.package_size`, shared, since migration 005 | Exists |
| basis name (`base_name`) — "pack", "breast", "scoop", "tbsp" | nothing | **The one schema change.** Migration 022, one shared `TEXT NOT NULL DEFAULT ''` on `food_items`. |
| `usual at <slot>` | `get_recent_foods` is most-**recent**; the card needs most-**frequent per slot** | New db function in Pack 3 |

Two things deliberately **not** added:

- **`last_grams` as a column.** It would duplicate the entry log.
- **Either new field on a per-user table.** `base_name` describes the food, not the
  eater, so it belongs on the shared catalog — but the mirror-image rule holds too:
  nothing personal goes back onto `food_items`. Migration 020 split `is_favourite`,
  `default_portion_g` and `custom_portions` off that shared row precisely because
  "two users would overwrite each other's answers on every toggle."

**Basis fallback chain:** `package_size` → `default_portion_g` (the user's usual) →
`100 g`, matching the prototype's `f.base || f.usual || 100`.

**Schema ritual reminder:** apply the migration to the local dev DB, then
`DATABASE_URL=sqlite:portfolio.db cargo sqlx prepare`, and commit `.sqlx/`.

## Algorithms, lifted from the prototype

Server-side in Rust; these are the prototype's own, not paraphrases.

- **Fraction buttons** (`chipsFor`): `base × {1, ½, ⅓, ¼}`, rounded to whole grams
  at ≥20 g else to 0.1 g; drop anything under 3 g; drop a duplicate within 0.5 g of
  one already emitted. Then append `last` if it is not within 0.5 g of an existing
  button. Selected = within 0.5 g of the row's current grams.
- **Nudge step** (`step`): 5 g below 50 g, otherwise 10 g. Floor 1 g, round to 0.1 g.
- **Calorie shares** (`shares`): protein ×4, carbs ×4, fat ×9, normalised; guard a
  zero denominator to 1 so a macro-less food yields no division by zero.
- **Dominance** (`dominance`): `needs_macros` ⇒ `no macros`; largest share under
  45% ⇒ `balanced`; else that macro. Drives the row's thumbnail ring, its label
  colour, and the library grouping.
- **Clock seeding** (unchanged from `defaultSlot()`): `<11` breakfast, `<15` lunch,
  `<17` snack, `<22` dinner, else snack. Seeds the picker on **first render only**.

## Accepted trade-offs and open rulings

- **The bridge repaints two out-of-scope surfaces.** `/fitness/week` and the Add
  sheet are `--noc-*` consumers, so Pack 1 changes their appearance: accent
  `#9184d9` → `#B48EF7`, radii 4/8/14 → 5/8/12, and the 1px ring "shadow" becomes a
  real border. The spec says those surfaces keep their *behaviour*; it says nothing
  about appearance. **Ruled 2026-08-17: let them repaint** — one palette across
  `/fitness`, no forked token set. Both surfaces join the Pack 1 walkthrough even
  though they are out of design scope.
- **`fresh` has no server-side home.** The spec calls it session-scoped and
  explicitly non-persistent, and there is no per-session store to put it in.
  Planned: the insert returns the new row id, the client marks that row fresh and
  derives the `N to check` count. Recorded here so it is a decision, not a
  discovery. **Consequence:** a full page reload clears the amber edges and resets
  `N to check` to zero. That is what "session-scoped" means here — not a bug.
- **The pack gate needs a real browser and this environment fought it.** The
  in-app browser pane would not composite (`screenshot` timed out — pane not
  displayed), and past sessions hit Dark Reader repainting colours and
  `resize_window` not taking. Every pack here closes on a visual walkthrough, so
  budget for the pane being uncooperative rather than meeting it at the gate.
- **The workspace test count needs re-measuring.** CLAUDE.md's 792 was measured on
  the Last Call branch; `dev` will differ. Take the number from
  `cargo test --workspace` at the first pack gate and correct CLAUDE.md there.
- **Icons are a flagged substitution.** The spec names Lucide v0.462.0
  (`command`, `search`, `scan-line`, `calendar-days`, `chevron-left`,
  `sliders-horizontal`, `copy`); `static/icons/` ships three SVGs. The README says
  swap freely — only the `Icon` wrapper changes.
- **Food thumbnails** are letter tiles in the prototype by design. Real images come
  from each item's existing `image_url`; keep the ring colour and the 34/30/26px
  sizes and swap the letter for the image where one exists.
- **`can_edit: bool`** threads through `day_section_html` and must thread through
  every new fragment builder. It is a permissions surface, not a display flag.
- **Askama auto-escapes; these builders do not.** `nutrition.rs` carries its own
  private `html_escape()` (`:15`) because the fragments are format strings. Every
  new fragment builder keeps using it.

## Ledger

### Pack 1 — done 2026-08-17

All four items landed. **Pack gate green:** `./scripts/verify.sh` — fmt, clippy,
833 workspace tests, JS syntax all clean. Clippy holds at 19 distinct warnings,
none in new code. Test count 830 → 833 (the three new date-helper tests);
CLAUDE.md corrected.

**The pack shrank on contact with the tree.** The artportfolio slice had already
landed the entire token layer on bare `:root` — all five groups, the exact
`@font-face` set, and the primitives whose class names the prototype's markup
uses (`.hm-btn`, `.hm-icon`, `.hm-kbd`). Items 1.1 and 1.2 stopped being "write
a token layer" and became "unscope one" and "delete a link".

| Item | Landed as | Note |
| --- | --- | --- |
| 1.1 tokens | `1be6c2e` | 61 primitive rules `body.art-page` → `:is(body.art-page, body.fitness-dark)` |
| 1.2 type | `34864f6` | Inter dropped from both fitness templates; **no font files copied** |
| 1.3 bridge | `bbb7f14` | `--noc-*` declares no hex at all now |
| 1.4 shell | `02b3753` | + 8 chrome rules generalised; 2 Rust helpers, 3 tests |

<details>
<summary><b>Walkthrough evidence</b> — computed styles, not eyeballs</summary>

The browser pane would not composite (`screenshot` times out — the pane is not
displayed in this environment, and `file://` URLs render as static snapshots).
Verified instead by running the real server and reading `getComputedStyle` in
the live page, which is stronger evidence for CSS work than a screenshot: it
asserts values rather than impressions. **A human still has to look at it** —
this proves the rules resolve, not that it looks right.

*`/fitness` at 1280px* — every value matches the spec: page `max-width` 1180px,
columns `row`, left rail `sticky` / `top: 80px` / `max-width: 360px`, right
column `2 1 480px`, `h1` Archivo 800 40px, micro-label IBM Plex Mono with
1.1px tracking (= 0.10em × 11px), header `sticky` 56px, body `rgb(11, 9, 16)`
(`#0B0910`).

*Bridge working:* `--noc-accent` resolves to `#B48EF7` (was `#9184d9`) and
`--noc-radius-md` to 8px, on all three `body.fitness-dark` pages.

*No bleed* — the item flagged for individual review. `/`, `/tasks` and `/admin`
all still compute `rgb(245, 245, 245)`, `system-ui`, `position: static` header.
`/artportfolio` is unchanged **and** has no `--noc-*` set at all, confirming the
bridge is fitness-scoped and the `:is()` change is purely additive.

*Type* — `document.fonts` shows Archivo 700/800 and Space Grotesk 400/500
loaded from `/static/fonts`; `performance.getEntriesByType('resource')` lists
**zero** off-origin requests. (IBM Plex Mono 400 reads "unloaded" simply because
no `--type-mono` text renders until Pack 2.)

*Breakpoint* — measured at 899/900/917/920px: `flex-direction` tracks
`matchMedia('(min-width: 900px)')` exactly. The apparent early flip at a 900px
frame is the iframe scrollbar (innerWidth 896), not a CSS fault.

Local setup, for whoever repeats this: `.env` from `.env.example` with
`RP_ID=localhost`; the seeded owner (id 1, "admin") has **no** PIN, so logging
in needs one written into `users.pin_hash` — generate it with `pin::hash_pin`
rather than by hand, since it is a salted Argon2 PHC string. `/api/auth/login/pin`
takes **JSON**, not form encoding.

</details>

<details>
<summary><b>Deviations from the spec, and what Pack 2 inherits</b></summary>

- **The header is not the prototype's.** It keeps base.html's "Portfolio"
  wordmark and bare `Ctrl`/`K` keycaps rather than the `hampter` wordmark with a
  violet full stop and a labelled Search button. Those are markup changes to
  `base.html` **and** `admin.html` — shared with `/admin`, `/tasks` and the hub —
  so they are out of scope for a fitness pack. Recorded in the stylesheet at the
  chrome block. Pack 5 or a follow-up should decide whether to take them.
- **`--shadow-3` is loose in the fitness pages.** `--noc-shadow-pop` now resolves
  to an 18px/48px shadow where it was a 1px ring plus a 6px shadow. Every popover
  on `/fitness`, `/fitness/week` and `/fitness/account` wears it. Computed styles
  cannot judge whether it is too heavy — this is the first thing to look at when
  a human opens the page.
- **`.fitness-page` is still the theme marker.** `base.html`'s `syncBodyTheme()`
  keys the `fitness-dark` body class off `main .fitness-page`, so that class must
  stay the outermost element in the content block. Noted in the template; a Pack
  2 restructure that moves it silently breaks the theme on boosted navigation
  only — never on a cold load, which is why it would survive a casual check.
- **`.claude/launch.json` was added** so the app can be started from the
  preview tooling.

</details>

<details>
<summary><b>Review wave — 3 findings, all fixed</b> (<code>high</code>, 2026-08-17)</summary>

All three were regressions this pack introduced by sharing artportfolio's CSS,
and all three were confirmed in the running app rather than argued from the
diff. Fixed in `43b3f5e`.

1. **`/fitness/week` widened 760px → 1180px.** The shell was keyed on
   `.fitness-page`, which the week template also carries
   (`<section class="fitness-page week-page">`), and the item-1.4 commit had
   deleted the 760px clamp that used to hold it. The week view was ruled in for
   a *repaint*, not a re-layout. Fixed by giving the Today page its own
   `.fitness-today` hook and restoring the clamp scoped to `.week-page`.
   `.fitness-page` stays on both — `syncBodyTheme()` keys the theme off it.
2. **`/fitness` gained a horizontal scrollbar on phones** — 423px of content in
   a 371px viewport, the overflowing element being base.html's Ctrl/K button
   pushed out of the now-unwrappable 56px header. Fixed with the brief's own
   phone rule, hiding the nav links below 900px.
3. **Anchor-styled buttons underlined on hover.** `a:hover` is (0,2,1) and
   `.hm-btn` only (0,1,1), so the reset's underline beat the button's
   `text-decoration: none`. Fixed by restating it at `:hover` parity, which
   covers every variant.

**Worth carrying forward — the fix for 2 failed on the first attempt**, in a way
that looked like it had worked. Written into the fitness section,
`body.fitness-dark header nav` ties the chrome's `:is(...) header nav` at
(0,1,3); a media query contributes no specificity; and the chrome rule sits
~800 lines later, so it won and the overflow survived. An override of a shared
primitive has to sit **after** the rule it overrides, not in the section that
conceptually owns it. Re-measuring rather than trusting the edit is what caught
it.

`/artportfolio` still overflows at 390px. That predates this work and fixing it
would mean deleting its mobile navigation — not a fitness pack's call. Left
recorded here rather than silently changed.

</details>
