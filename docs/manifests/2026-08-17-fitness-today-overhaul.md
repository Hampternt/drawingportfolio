# Container: Fitness "Today" overhaul

**Status:** planned — awaiting go. No pack started.
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
<summary><b>Pack 1 — Token layer + page shell</b> (planned)</summary>

**Done when:** `/fitness` renders on the Hampter Design System tokens, self-hosted
type, and the overhaul's desktop frame — sticky header, page head, two columns —
with the existing cards sitting inside it, and no other page changes appearance.

- [ ] **1.1 Token layer, scoped.** Paste the `:root` blocks from `tokens/colors.css`,
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
- [ ] **1.2 Type wired; Inter dropped.** **No font files to copy** — the roles this
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
- [ ] **1.3 Nocturne bridge.** Add the `legacy-nocturne.css` mapping block so every
      `--noc-*` resolves to a new token.
      *Done: `/fitness`, `/fitness/week` and the Add sheet repaint with zero markup
      changes.* See **Accepted trade-offs** — this repaints two out-of-scope surfaces.
- [ ] **1.4 Page shell.** Sticky 56px header (wordmark with the violet stop, nav,
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
<summary><b>Packs 2–5 — scope only</b></summary>

**Pack 2 — The logged row.** The core of the redesign. Quantity moves onto the row:
`full / ½ / ⅓ / ¼` of the food's basis, `last`, and a `custom` nudge row. Row macro
pie, per-macro colour coding, dominance label and basis line. Fresh-row amber edge.
Fixes the slot-reset bug. Changes fragment boundaries to per-row swaps — which
**breaks the week-strip refresh** at `feed.html:380` (it keys on
`e.target.id === 'day-section'`, which will no longer fire); that is an item in this
pack, not a discovery for the gate. Carries the one schema change (see below).

**Pack 3 — One-tap logging.** `Log again` chips, `usual at <slot>`, create-and-log
from the search field, saved-meal batch logging, the meal builder, and the toast
with Undo. Deletes the food dropdown and with it bug 2.

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

Nothing executed yet.
