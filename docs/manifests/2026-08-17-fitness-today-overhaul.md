# Container: Fitness "Today" overhaul

**Status:** Pack 1 landed 2026-08-17 (gate green, walkthrough below). Pack 2 next.
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
