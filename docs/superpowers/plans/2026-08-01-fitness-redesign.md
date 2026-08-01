# Fitness Tracker Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rebuild `/fitness` as the phone-first, Nocturne-themed tracker designed in `docs/design/fitness-redesign/` — targets with a calorie ring, meal slots, a scan-first add sheet, a grouped library, and a week/weight view — behind the passkey session.

**Architecture:** Same stack and layering as today: SQL only in `src/db.rs`, HTML fragments as Rust format strings in `src/routes/nutrition.rs`, one Askama page template per route, HTMX swaps, styles in `static/style.css` under named section comments. Four additive migrations (008–011). The whole `/fitness` section becomes session-gated (`AuthSession`).

**Tech Stack:** Rust, Axum 0.8, sqlx 0.8 (compile-time `query!` macros + committed `.sqlx` offline cache), Askama 0.15, HTMX, SQLite, vanilla JS.

## Global Constraints

- chrono is pinned to `0.4.34` — use `Duration`, never `TimeDelta`.
- Styles go in `static/style.css` under a named section comment. Never add `<style>` blocks to templates that extend `base.html`.
- Any JS that injects DOM nodes must listen on **both** `DOMContentLoaded` and `htmx:afterSwap`, guarded with an existence check (hx-boost keeps the page alive across navigations).
- SQL queries belong in `db.rs` only; handlers call db functions.
- Templates receive pre-computed values; no logic beyond simple conditionals.
- Nocturne rules: accent `#9184d9` as line and glow, never a flood; primary buttons are accent-outlined, not filled; no pure black/white; hierarchy by size and space, max font-weight 600; every interactive element ≥44px tall on touch layouts; `:focus-visible` ring `2px solid var(--noc-accent)`.
- All new interface colors/spacing/radii come from the `--noc-*` token variables added in Task 2 — no new hard-coded hexes outside the token block (SVG chart strokes inside format strings may use the literal hex of a token, commented).
- Meal slot values are exactly: `breakfast`, `lunch`, `dinner`, `snack`, `other`.
- Clock-inferred slot (client local time): hour < 11 → breakfast, < 15 → lunch, < 17 → snack, < 22 → dinner, else snack.
- Verification for every task: `cargo fmt --check && cargo clippy && SQLX_OFFLINE=true cargo test` all green before commit. Quote failing output if any.

## Known divergences from the design mockups (deliberate, decided 2026-08-01)

- **First-scan flow (mockup 2c):** an unknown barcode falls back to the existing OpenFoodFacts-prefilled add-food form instead of a one-step "save default & log" sheet. The per-food default portion is set in the food detail form (or on first edit); the second scan onward is the two-tap path. Full 2c one-step flow is future work.
- **Desktop (mockup 3a):** the three-column app shell is not built; desktop gets the same column wider plus the keyboard quick-add (the design's own principle: "the desktop view is the same column, wider").
- **`meal_entries.logged_at`:** not added — `created_at` already records it.
- **`recent_foods` SQL view:** implemented as a query (`get_recent_foods`), not a view.

## Build ritual for schema-touching tasks (sqlx compile-time macros)

The repo uses `sqlx::query!` macros checked against a live schema, with a committed `.sqlx` offline cache. Any task that adds a migration or changes a query MUST:

```bash
# once per worktree (Task 1 does it): local dev DB with current schema
cp ~/projects/drawingportfolio/portfolio.db ./portfolio.db   # gitignored

# after writing migrations/NNN_*.sql, apply it to the dev DB (no sqlite3 CLI on this machine):
python3 -c "import sqlite3; sqlite3.connect('portfolio.db').executescript(open('migrations/008_meal_slots.sql').read())"

# after writing/changing query! code — regenerate the offline cache:
DATABASE_URL=sqlite:portfolio.db cargo sqlx prepare

# tests and builds run offline:
SQLX_OFFLINE=true cargo test
```

`git add .sqlx` with the task's commit — the cache is part of the repo.

## Dev-session ritual (the page is auth-gated after Task 1)

To view `/fitness` in a browser against the dev server:

```bash
python3 -c "import sqlite3; c=sqlite3.connect('portfolio.db'); c.execute(\"INSERT OR REPLACE INTO sessions (id, expires_at) VALUES ('devsession','2099-01-01T00:00:00')\"); c.commit()"
```

then in the browser devtools console on `http://localhost:3000`:
`document.cookie = "session=devsession; path=/"`.

For automated checks without a browser: `curl -s -H "Cookie: session=devsession" http://localhost:3000/fitness`.

## File structure

- `migrations/008_meal_slots.sql` … `migrations/011_weights_recipes.sql` — new, one per schema step.
- `src/db.rs` — new nutrition query functions + their tests (existing file, existing pattern).
- `src/models.rs` — `Targets`, `RecentFood`, `RecipeWithTotals` structs; `MealEntryWithFood` and `FoodItem` gain fields.
- `src/routes/nutrition.rs` — handlers + all fragment builders (`day_section_html`, `library_list_html`, `add_sheet_html`, `week_page` …). Stays one file (repo pattern: one module per feature area).
- `templates/fitness/feed.html` — Today page (rewritten markup + page JS).
- `templates/fitness/week.html` — new Week page.
- `templates/base.html` — gains a `{% block body_class %}` hook on `<body>` (admin.html is standalone and does not need this hook — recorded exception stays).
- `static/style.css` — new `/* ── Nocturne tokens (fitness) ── */` block + full rewrite of the `/* ── Fitness Tracker ── */` section.
- `static/barcode.js` — rewired to the add sheet.
- `static/palette.js` — new command for the week view.
- Reference (read, never edit): `docs/design/fitness-redesign/redesign-mockups.html` (screens 1a–1e, 2a–2d, 3a), `nocturne-tokens.css`, `nocturne-readme.md`, `redesign-plan.md`.

---

### Task 1: Gate /fitness behind the session

**Files:**
- Modify: `src/routes/nutrition.rs` (handlers `fitness_page`, `htmx_day`, `add_food_item`, `add_meal_entry`)
- Modify: `templates/fitness/feed.html` (drop the `{% if is_admin %}` guard around the image field)

**Interfaces:**
- Consumes: `crate::middleware::AuthSession` (existing extractor; rejects with a redirect to `/admin/login`).
- Produces: every `/fitness*` and `/api/nutrition/*` handler requires `AuthSession`. Fragment builders keep their `is_admin: bool` parameter; handlers now always pass `true`.

- [ ] **Step 1: Set up the dev DB copy (build ritual)**

```bash
cp ~/projects/drawingportfolio/portfolio.db ./portfolio.db
```

- [ ] **Step 2: Switch the four OptionalAuth handlers to AuthSession**

In `src/routes/nutrition.rs`, change the extractor on `fitness_page`, `htmx_day`, `add_food_item`, `add_meal_entry` from `OptionalAuth(is_admin): OptionalAuth` to `AuthSession(_): AuthSession`, and replace every use of the old `is_admin` variable in those bodies with the literal `true`. Example for `fitness_page`:

```rust
async fn fitness_page(
    AuthSession(_): AuthSession,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let entries = crate::db::get_meal_entries_for_date(&state.pool, &today).await;
    let food_items = crate::db::get_food_items(&state.pool).await;
    let day_html = day_section_html(&entries, &today, &food_items, true);
    let lib_html = library_list_html(&food_items, true);
    Html(FitnessTemplate {
        is_admin: true,
        today,
        day_section_html: day_html,
        library_html: lib_html,
    }.render().unwrap())
}
```

In `add_food_item`, the `if is_admin { … }` wrapper around the S3 upload goes away (the body runs unconditionally). Remove the now-unused `OptionalAuth` import if nothing else in the file uses it.

- [ ] **Step 3: Drop the template guard**

In `templates/fitness/feed.html`, replace

```html
      {% if is_admin %}
      <label class="file-label">Image <input type="file" name="image" accept="image/jpeg,image/png,image/webp"></label>
      {% endif %}
```

with

```html
      <label class="file-label">Image <input type="file" name="image" accept="image/jpeg,image/png,image/webp"></label>
```

(The `is_admin` template field stays — `base.html` reads it for `IS_ADMIN`.)

- [ ] **Step 4: Verify**

Run: `cargo fmt --check && cargo clippy && SQLX_OFFLINE=true cargo test`
Expected: all green; no behavior change for logged-in users. Then start `cargo run` and confirm `curl -s -o /dev/null -w "%{http_code}" http://localhost:3000/fitness` prints `303` (redirect) without a cookie and `200` with the dev-session cookie (see ritual above).

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(fitness): gate tracker behind passkey session"
```

---

### Task 2: Nocturne visual layer (tokens + full restyle, no schema change)

**Files:**
- Modify: `templates/base.html` (add `{% block body_class %}` hook)
- Modify: `templates/fitness/feed.html` (set the class, add Inter font link in `{% block head %}`)
- Modify: `static/style.css` (new token block; rewrite the Fitness Tracker section)

**Interfaces:**
- Produces: CSS custom properties `--noc-bg #161826`, `--noc-surface #232532`, `--noc-surface-2 #1c1e2b`, `--noc-text #e9e9ed`, `--noc-accent #9184d9`, `--noc-accent-300 #d2cefd`, `--noc-accent-400 #b5abfc`, `--noc-accent-900 #2b2741`, `--noc-n700 #595d6c`, `--noc-n800 #3f424d`, `--noc-n900 #292b31`, `--noc-divider`, `--noc-radius-sm 4px`, `--noc-radius-md 8px`, `--noc-radius-lg 14px`, `--noc-shadow-card`, plus classes `.noc-btn`, `.noc-btn-primary`, `.noc-btn-secondary`, `.noc-btn-ghost`, `.noc-btn-icon`, `.noc-tag`, `.noc-tag-accent`, `.noc-tag-outline`, `.noc-tag-neutral`, `.noc-input`, `.noc-card`, `.noc-kicker`. Every later task's markup uses these names — do not rename.
- Produces: `<body class="fitness-dark">` on fitness pages; all fitness styles are scoped under `body.fitness-dark`.

- [ ] **Step 1: Add the body_class hook to base.html**

In `templates/base.html` change `<body hx-boost="true">` to:

```html
<body hx-boost="true" class="{% block body_class %}{% endblock %}">
```

(admin.html is standalone — no parity change needed; note stays in CLAUDE.md.)

- [ ] **Step 2: Wire the fitness template into the hook and load Inter**

At the top of `templates/fitness/feed.html` (after the title block):

```html
{% block body_class %}fitness-dark{% endblock %}

{% block head %}
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600&display=swap">
{% endblock %}
```

- [ ] **Step 3: Add the token block and rewrite the fitness CSS section**

In `static/style.css`, immediately before the existing `/* ── Fitness Tracker ── */` section, add the token block, then replace the entire old Fitness Tracker section (from its section comment up to, not including, `/* ── Drawing Tasks ── */`) with the new one. Both blocks below are complete — paste as-is. Consult `docs/design/fitness-redesign/redesign-mockups.html` screen 1a for the look being targeted; `docs/design/fitness-redesign/nocturne-readme.md` for the rules.

```css
/* ── Nocturne tokens (fitness) ─────────────────── */
/* Ported subset of docs/design/fitness-redesign/nocturne-tokens.css,
   prefixed --noc- and scoped to the fitness pages via body.fitness-dark. */
body.fitness-dark {
  --noc-bg: #161826;
  --noc-surface: #232532;
  --noc-surface-2: #1c1e2b;
  --noc-text: #e9e9ed;
  --noc-accent: #9184d9;
  --noc-accent-300: #d2cefd;
  --noc-accent-400: #b5abfc;
  --noc-accent-900: #2b2741;
  --noc-n700: #595d6c;
  --noc-n800: #3f424d;
  --noc-n900: #292b31;
  --noc-divider: color-mix(in srgb, #e9e9ed 16%, transparent);
  --noc-muted: color-mix(in srgb, #e9e9ed 55%, transparent);
  --noc-faint: color-mix(in srgb, #e9e9ed 45%, transparent);
  --noc-radius-sm: 4px;
  --noc-radius-md: 8px;
  --noc-radius-lg: 14px;
  --noc-shadow-card: 0 0 0 1px #3f424d;
  --noc-shadow-pop: 0 0 0 1px #595d6c, 0 6px 18px rgba(0,0,0,0.55);

  background: var(--noc-bg);
  color: var(--noc-text);
  font-family: "Inter", system-ui, sans-serif;
}
body.fitness-dark header {
  background: var(--noc-bg);
  border-bottom: 1px solid var(--noc-divider);
}
body.fitness-dark header .site-title,
body.fitness-dark header nav a { color: var(--noc-text); }
body.fitness-dark header nav a:hover { color: var(--noc-accent); }
body.fitness-dark ::selection { background: color-mix(in srgb, var(--noc-accent) 30%, transparent); }
body.fitness-dark :focus-visible { outline: 2px solid var(--noc-accent); outline-offset: 2px; }

/* Nocturne primitives (fitness-scoped) */
body.fitness-dark .noc-btn {
  display: inline-flex; align-items: center; justify-content: center; gap: 6px;
  cursor: pointer; font: inherit; font-weight: 500; font-size: 14px; line-height: 1.2;
  color: var(--noc-text); background: transparent;
  border: 1px solid transparent; border-radius: var(--noc-radius-md);
  min-height: 44px; padding: 8px 14px;
}
body.fitness-dark .noc-btn:disabled { opacity: .45; cursor: not-allowed; }
body.fitness-dark .noc-btn-primary { color: var(--noc-accent); border-color: var(--noc-accent); }
body.fitness-dark .noc-btn-primary:hover { background: color-mix(in srgb, var(--noc-accent) 12%, transparent); }
body.fitness-dark .noc-btn-primary:active { background: color-mix(in srgb, var(--noc-accent) 22%, transparent); }
body.fitness-dark .noc-btn-secondary { border-color: var(--noc-divider); }
body.fitness-dark .noc-btn-secondary:hover { background: color-mix(in srgb, var(--noc-text) 7%, transparent); }
body.fitness-dark .noc-btn-secondary:active { background: color-mix(in srgb, var(--noc-text) 14%, transparent); }
body.fitness-dark .noc-btn-ghost { color: var(--noc-accent); }
body.fitness-dark .noc-btn-ghost:hover { background: color-mix(in srgb, var(--noc-accent) 10%, transparent); }
body.fitness-dark .noc-btn-icon { width: 44px; padding: 0; }
body.fitness-dark .noc-input {
  width: 100%; min-height: 44px; padding: 6px 12px; font: inherit; font-size: 14px;
  color: var(--noc-text); caret-color: var(--noc-accent);
  background: var(--noc-surface);
  border: 1px solid var(--noc-divider); border-radius: var(--noc-radius-md);
}
body.fitness-dark .noc-input:hover { border-color: color-mix(in srgb, var(--noc-text) 45%, transparent); }
body.fitness-dark .noc-input:focus-visible { border-color: var(--noc-accent); outline-offset: 0; }
body.fitness-dark .noc-tag {
  display: inline-flex; align-items: center; font-size: 11px; letter-spacing: .02em;
  padding: 6px 11px; border-radius: 6px; border: 1px solid transparent;
  background: none; cursor: pointer; font-family: inherit; color: var(--noc-text);
}
body.fitness-dark .noc-tag-accent { background: var(--noc-accent-900); color: var(--noc-accent-300); border-color: var(--noc-accent); }
body.fitness-dark .noc-tag-outline { border-color: var(--noc-accent); color: var(--noc-accent); }
body.fitness-dark .noc-tag-neutral { background: var(--noc-n800); color: var(--noc-text); }
body.fitness-dark .noc-card {
  background: var(--noc-surface); border-radius: var(--noc-radius-lg);
  box-shadow: var(--noc-shadow-card); padding: 16px;
}
body.fitness-dark .noc-kicker {
  font-size: 11px; letter-spacing: .08em; text-transform: uppercase;
  color: var(--noc-muted);
}

/* ── Fitness Tracker ─────────────────────────────────── */
.fitness-page { max-width: 640px; margin: 0 auto; padding: 0 16px 90px; }

/* date header */
.fitness-date-nav { display: flex; align-items: center; gap: 8px; margin: 14px 0; }
.fitness-date-nav input[type="date"] {
  flex: 1; min-height: 44px; padding: 6px 12px; font: inherit; font-size: 14px;
  color: var(--noc-text); background: var(--noc-surface);
  border: 1px solid var(--noc-divider); border-radius: var(--noc-radius-md);
  color-scheme: dark;
}

/* day totals (pre-targets shape; Task 3 replaces this block's markup) */
body.fitness-dark .day-totals {
  display: flex; gap: 16px; flex-wrap: wrap; align-items: baseline;
  background: var(--noc-surface); border-radius: var(--noc-radius-lg);
  box-shadow: var(--noc-shadow-card); padding: 16px; margin-bottom: 14px;
  font-size: 14px;
}
body.fitness-dark .day-totals .total-cal { font-size: 22px; font-weight: 500; letter-spacing: -.015em; }
body.fitness-dark .day-totals .total-macro { color: var(--noc-muted); }

/* meal entries */
body.fitness-dark .meal-list { list-style: none; padding: 0; margin: 0 0 12px; }
body.fitness-dark .meal-entry {
  display: flex; align-items: center; gap: 10px; padding: 9px 0;
  border-bottom: 1px solid color-mix(in srgb, var(--noc-text) 10%, transparent);
  font-size: 14px;
}
body.fitness-dark .meal-entry .entry-name { flex: 1; min-width: 0; }
body.fitness-dark .meal-entry .entry-grams { color: var(--noc-muted); font-size: 12px; white-space: nowrap; }
body.fitness-dark .meal-entry .entry-cal { color: var(--noc-accent-300); font-size: 13px; min-width: 52px; text-align: right; white-space: nowrap; }

/* shared delete / small buttons inside fragments */
body.fitness-dark .food-delete-btn {
  background: none; border: none; cursor: pointer; color: var(--noc-faint);
  font-size: 17px; min-width: 44px; min-height: 44px; border-radius: var(--noc-radius-md);
}
body.fitness-dark .food-delete-btn:hover { color: var(--noc-accent-400); background: color-mix(in srgb, var(--noc-accent) 10%, transparent); }
body.fitness-dark .food-edit-btn {
  background: none; border: 1px solid var(--noc-divider); border-radius: var(--noc-radius-md);
  cursor: pointer; color: var(--noc-muted); font-size: 12px; font-family: inherit;
  min-height: 34px; padding: 4px 10px;
}
body.fitness-dark .food-edit-btn:hover { background: color-mix(in srgb, var(--noc-text) 7%, transparent); }

/* log form */
body.fitness-dark .log-entry-form { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; margin-top: 10px; }
body.fitness-dark .log-entry-form select,
body.fitness-dark .log-entry-form input[type="number"] {
  min-height: 44px; padding: 6px 10px; font: inherit; font-size: 14px;
  color: var(--noc-text); background: var(--noc-surface);
  border: 1px solid var(--noc-divider); border-radius: var(--noc-radius-md);
}
body.fitness-dark .log-entry-form select[name="food_item_id"] { flex: 1; min-width: 140px; }
body.fitness-dark .log-entry-form input[type="number"] { width: 84px; }
body.fitness-dark .log-entry-form .grams-label { color: var(--noc-muted); font-size: 13px; }
body.fitness-dark .log-entry-form select:disabled { opacity: .45; }

/* library */
body.fitness-dark .food-library-section { margin-top: 28px; }
body.fitness-dark .library-header { display: flex; align-items: center; justify-content: space-between; gap: 10px; flex-wrap: wrap; margin-bottom: 12px; }
body.fitness-dark .library-header h2 { margin: 0; font-size: 20px; font-weight: 500; letter-spacing: -.015em; }
body.fitness-dark .library-actions { display: flex; gap: 8px; flex-wrap: wrap; align-items: center; }
body.fitness-dark .library-actions input[type="text"] {
  min-height: 44px; padding: 6px 12px; font: inherit; font-size: 14px;
  color: var(--noc-text); background: var(--noc-surface);
  border: 1px solid var(--noc-divider); border-radius: var(--noc-radius-md);
}
body.fitness-dark .food-library-list { list-style: none; padding: 0; margin: 0; display: flex; flex-direction: column; gap: 8px; }
body.fitness-dark .food-item-card {
  display: flex; align-items: center; gap: 12px; padding: 10px;
  background: var(--noc-surface); border-radius: var(--noc-radius-md);
  box-shadow: var(--noc-shadow-card);
}
body.fitness-dark .food-thumb { width: 40px; height: 40px; border-radius: 6px; object-fit: cover; flex: none; background: var(--noc-accent-900); }
body.fitness-dark .food-info { flex: 1; min-width: 0; }
body.fitness-dark .food-info strong { display: block; font-size: 14px; font-weight: 500; }
body.fitness-dark .food-brand { color: var(--noc-muted); font-size: 12px; font-weight: 400; margin-left: 4px; }
body.fitness-dark .food-macros { font-size: 11.5px; color: var(--noc-muted); }
body.fitness-dark .food-pkg { margin-left: 6px; color: var(--noc-faint); font-size: 11px; }
body.fitness-dark .food-admin-btns { display: flex; gap: 4px; align-items: center; flex: none; }

/* add/edit food form */
body.fitness-dark .nutrient-form {
  display: flex; flex-direction: column; gap: 10px; padding: 16px;
  background: var(--noc-surface); border-radius: var(--noc-radius-lg);
  box-shadow: var(--noc-shadow-card); margin-bottom: 12px;
}
body.fitness-dark .nutrient-form input {
  min-height: 44px; padding: 6px 12px; font: inherit; font-size: 14px;
  color: var(--noc-text); background: var(--noc-surface-2);
  border: 1px solid var(--noc-divider); border-radius: var(--noc-radius-md);
  width: 100%;
}
body.fitness-dark .nutrient-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 8px; }
body.fitness-dark .nutrient-grid label,
body.fitness-dark .package-size-label,
body.fitness-dark .file-label { font-size: 12px; color: var(--noc-muted); display: flex; flex-direction: column; gap: 4px; }
body.fitness-dark .form-actions { display: flex; gap: 8px; }

/* legacy button classes still emitted by shared markup — restyle in scope */
body.fitness-dark .btn-primary {
  display: inline-flex; align-items: center; justify-content: center;
  cursor: pointer; font: inherit; font-weight: 500; font-size: 14px;
  color: var(--noc-accent); background: transparent;
  border: 1px solid var(--noc-accent); border-radius: var(--noc-radius-md);
  min-height: 44px; padding: 8px 16px;
}
body.fitness-dark .btn-primary:hover { background: color-mix(in srgb, var(--noc-accent) 12%, transparent); }
body.fitness-dark .btn-secondary {
  display: inline-flex; align-items: center; justify-content: center;
  cursor: pointer; font: inherit; font-size: 14px;
  color: var(--noc-text); background: transparent;
  border: 1px solid var(--noc-divider); border-radius: var(--noc-radius-md);
  min-height: 44px; padding: 8px 14px;
}
body.fitness-dark .btn-secondary:hover { background: color-mix(in srgb, var(--noc-text) 7%, transparent); }

/* barcode scanner */
body.fitness-dark #barcode-scanner video { width: 100%; border-radius: var(--noc-radius-lg); box-shadow: var(--noc-shadow-card); }
body.fitness-dark #scan-status { color: var(--noc-muted); font-size: 13px; }
```

- [ ] **Step 4: Verify**

Run: `cargo fmt --check && cargo clippy && SQLX_OFFLINE=true cargo test` (templates are compiled in — this catches Askama errors).
Expected: green. Then `cargo run`, open `http://localhost:3000/fitness` with the dev-session cookie and confirm: dark `#161826` page including the header, Inter type, card-shaped totals strip and library rows, outlined accent buttons, no browser-default focus ring. Screenshot-compare against `redesign-mockups.html` screen 1a for tone (structure changes later).

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(fitness): Nocturne visual layer — tokens and full restyle"
```

---

### Task 3: Targets + calorie ring (migration 009)

**Files:**
- Create: `migrations/009_targets.sql`
- Modify: `src/db.rs` (register migration, `get_targets`/`set_targets`, tests)
- Modify: `src/models.rs` (`Targets` struct)
- Modify: `src/routes/nutrition.rs` (`day_section_html` gains a targets param; ring + rails builders; targets edit endpoints)
- Modify: `templates/fitness/feed.html` (nothing structural — the fragment carries the change)
- Modify: `static/style.css` (day-summary styles, appended to the Fitness Tracker section)

**Interfaces:**
- Consumes: `--noc-*` tokens and `.noc-card`/`.noc-kicker`/`.noc-btn-ghost` classes from Task 2.
- Produces: `pub struct Targets { pub calories: f64, pub protein: f64, pub carbs: f64, pub fat: f64 }` in models; `db::get_targets(pool: &DbPool) -> Targets`, `db::set_targets(pool: &DbPool, calories: f64, protein: f64, carbs: f64, fat: f64)`; `day_section_html(entries: &[MealEntryWithFood], date: &str, food_items: &[FoodItem], targets: &Targets, is_admin: bool) -> String`; pure helpers `ring_offset(consumed: f64, target: f64) -> f64` and `rail_pct(value: f64, target: f64) -> f64` in `nutrition.rs`; routes `GET /fitness/htmx/targets?date=`, `POST /api/nutrition/targets`.

- [ ] **Step 1: Write the migration and apply it to the dev DB**

`migrations/009_targets.sql`:

```sql
CREATE TABLE IF NOT EXISTS targets (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    calories REAL NOT NULL,
    protein REAL NOT NULL,
    carbs REAL NOT NULL,
    fat REAL NOT NULL
);
INSERT OR IGNORE INTO targets (id, calories, protein, carbs, fat) VALUES (1, 2400, 165, 260, 72);
```

Register in `db::run_migrations` after the 007 block:

```rust
    // Migration 009: daily nutrition targets (single row, single user)
    let _ = sqlx::query(include_str!("../migrations/009_targets.sql"))
        .execute(pool)
        .await;
```

Apply to the dev DB (build ritual):

```bash
python3 -c "import sqlite3; sqlite3.connect('portfolio.db').executescript(open('migrations/009_targets.sql').read())"
```

- [ ] **Step 2: Write the failing db tests** (in `src/db.rs` `mod tests`)

```rust
    #[tokio::test]
    async fn test_targets_default_and_set() {
        let pool = test_pool().await;
        let t = get_targets(&pool).await;
        assert_eq!(t.calories, 2400.0);
        assert_eq!(t.protein, 165.0);
        set_targets(&pool, 2200.0, 170.0, 240.0, 70.0).await;
        let t = get_targets(&pool).await;
        assert_eq!(t.calories, 2200.0);
        assert_eq!(t.fat, 70.0);
    }
```

Run: `SQLX_OFFLINE=true cargo test test_targets_default_and_set`
Expected: FAIL — `get_targets` not found.

- [ ] **Step 3: Implement models + db functions**

`src/models.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Targets {
    pub calories: f64,
    pub protein: f64,
    pub carbs: f64,
    pub fat: f64,
}
```

`src/db.rs` (import `Targets` in the models use-list):

```rust
pub async fn get_targets(pool: &DbPool) -> Targets {
    sqlx::query_as!(Targets,
        r#"SELECT calories as "calories!", protein as "protein!", carbs as "carbs!", fat as "fat!" FROM targets WHERE id = 1"#
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
    .unwrap_or(Targets { calories: 2400.0, protein: 165.0, carbs: 260.0, fat: 72.0 })
}

pub async fn set_targets(pool: &DbPool, calories: f64, protein: f64, carbs: f64, fat: f64) {
    sqlx::query!(
        "INSERT INTO targets (id, calories, protein, carbs, fat) VALUES (1, ?, ?, ?, ?)
         ON CONFLICT(id) DO UPDATE SET calories = excluded.calories, protein = excluded.protein,
         carbs = excluded.carbs, fat = excluded.fat",
        calories, protein, carbs, fat
    )
    .execute(pool)
    .await
    .ok();
}
```

Then regenerate the cache and re-run: `DATABASE_URL=sqlite:portfolio.db cargo sqlx prepare && SQLX_OFFLINE=true cargo test test_targets_default_and_set`
Expected: PASS.

- [ ] **Step 4: Write failing unit tests for the ring/rail math** (new `mod tests` at the bottom of `src/routes/nutrition.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ring_offset_bounds() {
        // full ring left at zero consumed, empty at/beyond target
        assert!((ring_offset(0.0, 2400.0) - 263.9).abs() < 0.1);
        assert!(ring_offset(2400.0, 2400.0).abs() < 0.1);
        assert!(ring_offset(3000.0, 2400.0).abs() < 0.1);
        // 77% consumed → 23% of the circumference remains as offset
        assert!((ring_offset(1848.0, 2400.0) - 60.7).abs() < 0.5);
    }

    #[test]
    fn test_rail_pct_clamps() {
        assert_eq!(rail_pct(0.0, 165.0), 0.0);
        assert_eq!(rail_pct(330.0, 165.0), 100.0);
        assert!((rail_pct(122.0, 165.0) - 73.9).abs() < 0.2);
        assert_eq!(rail_pct(50.0, 0.0), 0.0);
    }
}
```

Run: `SQLX_OFFLINE=true cargo test test_ring_offset_bounds`
Expected: FAIL — `ring_offset` not defined.

- [ ] **Step 5: Implement the builders and rewire the day section**

In `src/routes/nutrition.rs`:

```rust
const RING_CIRC: f64 = 263.9; // 2π · 42 — matches the r="42" in calorie_ring_svg

fn ring_offset(consumed: f64, target: f64) -> f64 {
    let frac = if target > 0.0 { (consumed / target).clamp(0.0, 1.0) } else { 0.0 };
    RING_CIRC * (1.0 - frac)
}

fn rail_pct(value: f64, target: f64) -> f64 {
    if target <= 0.0 { return 0.0; }
    (value / target * 100.0).clamp(0.0, 100.0)
}

fn calorie_ring_svg(consumed: f64, target: f64) -> String {
    let offset = ring_offset(consumed, target);
    let remaining = (target - consumed).round();
    let (big, small) = if remaining >= 0.0 {
        (format!("{:.0}", remaining), "LEFT")
    } else {
        (format!("{:.0}", -remaining), "OVER")
    };
    // stroke hexes are the literal values of --noc-n800 / --noc-accent (SVG attrs can't read CSS vars from fragment strings)
    format!(
        r##"<svg class="cal-ring" width="98" height="98" viewBox="0 0 98 98" role="img" aria-label="{big} kcal {small_lc}">
  <circle cx="49" cy="49" r="42" fill="none" stroke="#3f424d" stroke-width="6"></circle>
  <circle cx="49" cy="49" r="42" fill="none" stroke="#9184d9" stroke-width="6" stroke-linecap="round" stroke-dasharray="{circ}" stroke-dashoffset="{offset:.1}" transform="rotate(-90 49 49)" style="filter:drop-shadow(0 0 6px rgba(145,132,217,.55))"></circle>
  <text x="49" y="46" text-anchor="middle" fill="#e9e9ed" font-size="21" font-weight="500">{big}</text>
  <text x="49" y="62" text-anchor="middle" fill="rgba(233,233,237,.5)" font-size="10" letter-spacing="0.08em">{small}</text>
</svg>"##,
        big = big, small = small, small_lc = small.to_lowercase(),
        circ = RING_CIRC, offset = offset
    )
}

fn macro_rail_html(label: &str, value: f64, target: f64, bar_hex: &str) -> String {
    format!(
        r##"<div class="macro-rail">
  <div class="rail-head"><span>{label}</span><span class="rail-nums">{v:.0} / {t:.0} g</span></div>
  <div class="rail-track"><div class="rail-fill" style="width:{pct:.0}%;background:{bar_hex}"></div></div>
</div>"##,
        label = label, v = value, t = target, pct = rail_pct(value, target), bar_hex = bar_hex
    )
}
```

In `day_section_html`, change the signature to

```rust
pub fn day_section_html(entries: &[crate::models::MealEntryWithFood], date: &str, food_items: &[crate::models::FoodItem], targets: &crate::models::Targets, is_admin: bool) -> String
```

and replace the `day-totals` div in the format string with the summary card (keep the entries list and form below unchanged for now — Task 4 restructures them):

```rust
    let pct_of_target = if targets.calories > 0.0 { (total_cal / targets.calories * 100.0).round() } else { 0.0 };
    let summary = format!(
        r##"<div class="day-summary noc-card">
  {ring}
  <div class="macro-rails">
    {p}{c}{f}
    <div class="cal-caption">{cal:.0} of {tcal:.0} cal · {pct:.0}%</div>
  </div>
</div>
<div class="targets-row">
  <button class="noc-btn noc-btn-ghost" hx-get="/fitness/htmx/targets?date={date}" hx-target="#targets-editor" hx-swap="innerHTML">Edit targets</button>
  <div id="targets-editor"></div>
</div>"##,
        ring = calorie_ring_svg(total_cal, targets.calories),
        p = macro_rail_html("Protein", total_protein, targets.protein, "#9184d9"),
        c = macro_rail_html("Carbs", total_carbs, targets.carbs, "#796cbf"),
        f = macro_rail_html("Fat", total_fat, targets.fat, "#5d5294"),
        cal = total_cal, tcal = targets.calories, pct = pct_of_target,
        date = html_escape(date)
    );
```

Every existing caller of `day_section_html` (in `fitness_page`, `htmx_day`, `add_meal_entry`, `delete_meal_entry_handler`) fetches targets first:

```rust
    let targets = crate::db::get_targets(&state.pool).await;
    // ...
    day_section_html(&entries, &date, &food_items, &targets, true)
```

Add the two new handlers and routes:

```rust
async fn targets_form(
    AuthSession(_): AuthSession,
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let date = params.get("date").cloned().unwrap_or_default();
    let t = crate::db::get_targets(&state.pool).await;
    Html(format!(
        r##"<form class="targets-form" hx-post="/api/nutrition/targets" hx-target="#day-section" hx-swap="innerHTML">
  <input type="hidden" name="date" value="{date}">
  <label>kcal<input class="noc-input" type="number" name="calories" min="0" step="1" value="{cal:.0}" required></label>
  <label>P g<input class="noc-input" type="number" name="protein" min="0" step="1" value="{p:.0}" required></label>
  <label>C g<input class="noc-input" type="number" name="carbs" min="0" step="1" value="{c:.0}" required></label>
  <label>F g<input class="noc-input" type="number" name="fat" min="0" step="1" value="{f:.0}" required></label>
  <button type="submit" class="noc-btn noc-btn-primary">Save</button>
</form>"##,
        date = html_escape(&date), cal = t.calories, p = t.protein, c = t.carbs, f = t.fat
    ))
}

async fn set_targets_handler(
    AuthSession(_): AuthSession,
    State(state): State<Arc<AppState>>,
    axum::Form(form): axum::Form<HashMap<String, String>>,
) -> impl IntoResponse {
    let g = |k: &str, d: f64| form.get(k).and_then(|v| v.parse().ok()).unwrap_or(d);
    crate::db::set_targets(&state.pool, g("calories", 2400.0), g("protein", 165.0), g("carbs", 260.0), g("fat", 72.0)).await;
    let date = form.get("date").cloned().unwrap_or_else(|| chrono::Utc::now().format("%Y-%m-%d").to_string());
    let targets = crate::db::get_targets(&state.pool).await;
    let entries = crate::db::get_meal_entries_for_date(&state.pool, &date).await;
    let food_items = crate::db::get_food_items(&state.pool).await;
    Html(day_section_html(&entries, &date, &food_items, &targets, true))
}
```

Routes added in `router()`:

```rust
        .route("/fitness/htmx/targets", get(targets_form))
        .route("/api/nutrition/targets", post(set_targets_handler))
```

- [ ] **Step 6: Append the CSS** (end of the Fitness Tracker section in `static/style.css`)

```css
/* day summary — ring + rails (Task 3) */
body.fitness-dark .day-summary { display: flex; gap: 18px; align-items: center; margin-bottom: 10px; }
body.fitness-dark .cal-ring { flex: none; }
body.fitness-dark .macro-rails { flex: 1; display: flex; flex-direction: column; gap: 11px; }
body.fitness-dark .macro-rail .rail-head { display: flex; justify-content: space-between; font-size: 12px; margin-bottom: 5px; }
body.fitness-dark .macro-rail .rail-nums { color: var(--noc-muted); white-space: nowrap; }
body.fitness-dark .rail-track { height: 5px; background: var(--noc-n800); border-radius: 3px; }
body.fitness-dark .rail-fill { height: 5px; border-radius: 3px; }
body.fitness-dark .cal-caption { font-size: 11px; color: var(--noc-faint); margin-top: 1px; }
body.fitness-dark .targets-row { margin-bottom: 12px; }
body.fitness-dark .targets-form { display: flex; gap: 8px; align-items: flex-end; flex-wrap: wrap; margin-top: 8px; }
body.fitness-dark .targets-form label { font-size: 11px; color: var(--noc-muted); display: flex; flex-direction: column; gap: 4px; flex: 1; min-width: 70px; }
```

- [ ] **Step 7: Verify**

Run: `cargo fmt --check && cargo clippy && SQLX_OFFLINE=true cargo test`
Expected: all green including the two new unit tests and the targets db test. Browser check with the dev session: ring renders with remaining kcal, three rails against targets, "Edit targets" round-trips.

- [ ] **Step 8: Commit**

```bash
git add -A && git commit -m "feat(fitness): daily targets, calorie ring and macro rails (migration 009)"
```

---

### Task 4: Meal slots + entry editing (migration 008)

**Files:**
- Create: `migrations/008_meal_slots.sql`
- Modify: `src/db.rs` (register migration — insert its block *above* 009's to keep numeric order; slot-aware queries; tests)
- Modify: `src/models.rs` (`MealEntry.slot`, `MealEntryWithFood.slot` + `food_item_id`)
- Modify: `src/routes/nutrition.rs` (grouped day section, slot on the log form, entry edit endpoints)
- Modify: `templates/fitness/feed.html` (`defaultSlot()` JS + slot chip behavior)
- Modify: `static/style.css` (slot groups, slot chips, entry edit row)

**Interfaces:**
- Consumes: `day_section_html(entries, date, food_items, targets, is_admin)` from Task 3; slot values and clock rule from Global Constraints.
- Produces: `db::insert_meal_entry(pool, food_item_id: i64, date: &str, grams: f64, slot: &str) -> Result<i64, sqlx::Error>` (**signature change** — update the two existing db tests and `add_meal_entry`); `db::update_meal_entry(pool, id: i64, grams: f64, slot: &str)`; `db::get_meal_entry(pool, id: i64) -> Option<MealEntry>`; `MealEntryWithFood` gains `pub slot: String` and `pub food_item_id: i64`; routes `PUT /api/nutrition/entries/{id}` and `GET /fitness/htmx/entries/{id}/edit?date=`; JS `defaultSlot()` and `setSlot(form, slot)` in `feed.html`; `SLOTS` const in `nutrition.rs`.

- [ ] **Step 1: Write the migration and apply it**

`migrations/008_meal_slots.sql`:

```sql
ALTER TABLE meal_entries ADD COLUMN slot TEXT NOT NULL DEFAULT 'other';
```

(The design plan also named a `logged_at` column; `created_at` already records the timestamp, so it is deliberately not added — divergence recorded here.)

Register in `run_migrations` **before** the 009 block:

```rust
    // Migration 008: meal slot per entry (breakfast/lunch/dinner/snack/other)
    let _ = sqlx::query(include_str!("../migrations/008_meal_slots.sql"))
        .execute(pool)
        .await;
```

Apply: `python3 -c "import sqlite3; sqlite3.connect('portfolio.db').executescript(open('migrations/008_meal_slots.sql').read())"`

- [ ] **Step 2: Write the failing db tests**

```rust
    #[tokio::test]
    async fn test_meal_entry_slot_roundtrip() {
        let pool = test_pool().await;
        let item = insert_food_item(&pool, "Skyr", "", None, 63.0, 11.0, 4.0, 0.2, 0.0, 4.0, 45.0, 0.1, None, "", "").await;
        let id = insert_meal_entry(&pool, item.id, "2026-08-01", 250.0, "breakfast").await.unwrap();
        let entries = get_meal_entries_for_date(&pool, "2026-08-01").await;
        assert_eq!(entries[0].slot, "breakfast");
        assert_eq!(entries[0].food_item_id, item.id);
        update_meal_entry(&pool, id, 300.0, "lunch").await;
        let entries = get_meal_entries_for_date(&pool, "2026-08-01").await;
        assert_eq!(entries[0].grams, 300.0);
        assert_eq!(entries[0].slot, "lunch");
        let raw = get_meal_entry(&pool, id).await.unwrap();
        assert_eq!(raw.slot, "lunch");
    }
```

Run: `SQLX_OFFLINE=true cargo test test_meal_entry_slot_roundtrip`
Expected: FAIL — wrong arity on `insert_meal_entry`, missing functions/fields.

- [ ] **Step 3: Implement models + db changes**

`src/models.rs` — add to `MealEntry`:

```rust
    pub slot: String,
```

and to `MealEntryWithFood`:

```rust
    pub food_item_id: i64,
    pub slot: String,
```

`src/db.rs`:

```rust
pub async fn insert_meal_entry(pool: &DbPool, food_item_id: i64, date: &str, grams: f64, slot: &str) -> Result<i64, sqlx::Error> {
    let id = sqlx::query!(
        "INSERT INTO meal_entries (food_item_id, date, grams, slot) VALUES (?, ?, ?, ?) RETURNING id",
        food_item_id, date, grams, slot
    )
    .fetch_one(pool)
    .await?
    .id;
    Ok(id.ok_or(sqlx::Error::RowNotFound)?)
}

pub async fn update_meal_entry(pool: &DbPool, id: i64, grams: f64, slot: &str) {
    sqlx::query!("UPDATE meal_entries SET grams = ?, slot = ? WHERE id = ?", grams, slot, id)
        .execute(pool)
        .await
        .ok();
}

pub async fn get_meal_entry(pool: &DbPool, id: i64) -> Option<crate::models::MealEntry> {
    sqlx::query_as!(crate::models::MealEntry,
        r#"SELECT id, food_item_id, date, grams, slot as "slot!", created_at FROM meal_entries WHERE id = ?"#, id
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
}
```

In `get_meal_entries_for_date`, add `me.food_item_id, me.slot as "slot!"` to the SELECT and map them into the struct (`food_item_id: r.food_item_id, slot: r.slot`). Update the two existing tests (`test_insert_meal_entry_and_get_for_date`, `test_delete_meal_entry`, `test_meal_entry_wrong_date_not_returned`) to pass `"other"` as the new final argument.

Regenerate cache, re-run: `DATABASE_URL=sqlite:portfolio.db cargo sqlx prepare && SQLX_OFFLINE=true cargo test`
Expected: PASS.

- [ ] **Step 4: Group the day section by slot and add the slot control to the form**

In `src/routes/nutrition.rs` add:

```rust
const SLOTS: [(&str, &str); 5] = [
    ("breakfast", "Breakfast"),
    ("lunch", "Lunch"),
    ("dinner", "Dinner"),
    ("snack", "Snack"),
    ("other", "Other"),
];
```

Replace the flat `<ul class="meal-list">…</ul>` part of `day_section_html` with a per-slot loop (the summary card from Task 3 stays above it, the log form below it):

```rust
    let slots_html: String = SLOTS.iter().map(|(key, label)| {
        let slot_entries: Vec<_> = entries.iter().filter(|e| e.slot == *key).collect();
        if slot_entries.is_empty() && *key == "other" {
            return String::new(); // "other" group hidden when empty
        }
        let slot_cal: f64 = slot_entries.iter().map(|e| e.calories).sum();
        let head_right = if slot_entries.is_empty() {
            r#"<span class="slot-cal slot-empty">empty</span>"#.to_string()
        } else {
            format!(r#"<span class="slot-cal">{} cal</span>"#, fmt_nutrient(slot_cal))
        };
        let body = if slot_entries.is_empty() {
            format!(
                r##"<button type="button" class="noc-btn noc-btn-secondary slot-add-btn" onclick="addToSlot('{key}')">+ Add to {label_lc}</button>"##,
                key = key, label_lc = label.to_lowercase()
            )
        } else {
            let rows: String = slot_entries.iter()
                .map(|e| meal_entry_row_html(e, date, is_admin))
                .collect::<Vec<_>>()
                .join("\n");
            format!("<ul class=\"meal-list\">\n{}\n</ul>", rows)
        };
        format!(
            r##"<div class="slot-group" id="slot-{key}">
  <div class="slot-head"><span class="noc-kicker">{label}</span>{head_right}</div>
  {body}
</div>"##,
            key = key, label = label, head_right = head_right, body = body
        )
    }).collect::<Vec<_>>().join("\n");
```

`meal_entry_row_html` gains an edit affordance — replace its format string with:

```rust
    format!(
        r#"<li class="meal-entry" id="entry-{id}">
  <button type="button" class="entry-main" hx-get="/fitness/htmx/entries/{id}/edit?date={date}" hx-target="#entry-{id}" hx-swap="outerHTML">
    <span class="entry-name">{name}</span>
    <span class="entry-grams">{grams}g</span>
    <span class="entry-cal">{cal}</span>
  </button>
  {delete_btn}
</li>"#,
        id = entry.entry_id, date = html_escape(date),
        name = html_escape(&entry.food_name),
        grams = fmt_nutrient(entry.grams), cal = fmt_nutrient(entry.calories),
        delete_btn = delete_btn
    )
```

The log form gains slot chips + hidden input (inside the existing `<form class="log-entry-form" …>`, before the submit button):

```html
  <input type="hidden" name="slot" value="other">
  <div class="slot-chips" data-role="slot-chips">
    <button type="button" class="noc-tag noc-tag-outline" data-slot="breakfast" onclick="setSlot(this)">Breakfast</button>
    <button type="button" class="noc-tag noc-tag-outline" data-slot="lunch" onclick="setSlot(this)">Lunch</button>
    <button type="button" class="noc-tag noc-tag-outline" data-slot="dinner" onclick="setSlot(this)">Dinner</button>
    <button type="button" class="noc-tag noc-tag-outline" data-slot="snack" onclick="setSlot(this)">Snack</button>
  </div>
```

- [ ] **Step 5: Add the edit/update handlers and routes**

```rust
fn entry_edit_row_html(entry: &crate::models::MealEntry, food_name: &str, date: &str) -> String {
    let slot_opts: String = SLOTS.iter()
        .filter(|(k, _)| *k != "other" || entry.slot == "other")
        .map(|(k, l)| format!(
            "<option value=\"{k}\"{sel}>{l}</option>",
            k = k, l = l,
            sel = if entry.slot == *k { " selected" } else { "" }
        ))
        .collect();
    format!(
        r##"<li class="meal-entry meal-entry-edit" id="entry-{id}">
<form hx-put="/api/nutrition/entries/{id}" hx-target="#day-section" hx-swap="innerHTML">
  <input type="hidden" name="date" value="{date}">
  <span class="entry-name">{name}</span>
  <input class="noc-input" type="number" name="grams" value="{grams}" min="1" max="5000" step="0.1" required>
  <select class="noc-input" name="slot">{slot_opts}</select>
  <button type="submit" class="noc-btn noc-btn-primary">Save</button>
  <button type="button" class="noc-btn noc-btn-ghost" hx-get="/fitness/htmx/day?date={date}" hx-target="#day-section" hx-swap="innerHTML">Cancel</button>
</form>
</li>"##,
        id = entry.id, date = html_escape(date), name = html_escape(food_name),
        grams = fmt_nutrient(entry.grams), slot_opts = slot_opts
    )
}

async fn entry_edit_form(
    AuthSession(_): AuthSession,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let date = params.get("date").cloned().unwrap_or_default();
    match crate::db::get_meal_entry(&state.pool, id).await {
        Some(entry) => {
            let name = crate::db::get_food_item(&state.pool, entry.food_item_id).await
                .map(|f| f.name).unwrap_or_default();
            Html(entry_edit_row_html(&entry, &name, &date)).into_response()
        }
        None => (StatusCode::NOT_FOUND, Html("<p>Entry not found</p>".to_string())).into_response(),
    }
}

async fn update_meal_entry_handler(
    AuthSession(_): AuthSession,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    axum::Form(form): axum::Form<HashMap<String, String>>,
) -> impl IntoResponse {
    let grams: f64 = form.get("grams").and_then(|v| v.parse().ok()).unwrap_or(0.0);
    let slot = form.get("slot").cloned().unwrap_or_else(|| "other".to_string());
    let slot = if SLOTS.iter().any(|(k, _)| *k == slot) { slot } else { "other".to_string() };
    if grams > 0.0 {
        crate::db::update_meal_entry(&state.pool, id, grams, &slot).await;
    }
    let date = form.get("date").cloned().unwrap_or_else(|| chrono::Utc::now().format("%Y-%m-%d").to_string());
    let targets = crate::db::get_targets(&state.pool).await;
    let entries = crate::db::get_meal_entries_for_date(&state.pool, &date).await;
    let food_items = crate::db::get_food_items(&state.pool).await;
    Html(day_section_html(&entries, &date, &food_items, &targets, true))
}
```

`add_meal_entry` reads and validates the slot the same way and passes it to `insert_meal_entry`. Routes:

```rust
        .route("/api/nutrition/entries/{id}", delete(delete_meal_entry_handler).put(update_meal_entry_handler))
        .route("/fitness/htmx/entries/{id}/edit", get(entry_edit_form))
```

- [ ] **Step 6: Page JS — clock-inferred default + chip behavior** (in `feed.html`'s script block)

```js
function defaultSlot() {
  const h = new Date().getHours();
  if (h < 11) return 'breakfast';
  if (h < 15) return 'lunch';
  if (h < 17) return 'snack';
  if (h < 22) return 'dinner';
  return 'snack';
}

function paintSlotChips(form) {
  const val = form.querySelector('[name="slot"]').value;
  form.querySelectorAll('[data-slot]').forEach(b => {
    b.classList.toggle('noc-tag-accent', b.dataset.slot === val);
    b.classList.toggle('noc-tag-outline', b.dataset.slot !== val);
  });
}

function setSlot(btn) {
  const form = btn.closest('form');
  form.querySelector('[name="slot"]').value = btn.dataset.slot;
  paintSlotChips(form);
}

function addToSlot(slot) {
  const form = document.querySelector('.log-entry-form');
  form.querySelector('[name="slot"]').value = slot;
  paintSlotChips(form);
  form.querySelector('[name="food_item_id"]').focus();
}

function initSlotDefault() {
  document.querySelectorAll('.log-entry-form').forEach(form => {
    const input = form.querySelector('[name="slot"]');
    if (input && input.value === 'other') { input.value = defaultSlot(); }
    paintSlotChips(form);
  });
}
document.addEventListener('DOMContentLoaded', initSlotDefault);
document.body.addEventListener('htmx:afterSwap', initSlotDefault);
```

- [ ] **Step 7: CSS additions**

```css
/* meal slots (Task 4) */
body.fitness-dark .slot-group { margin-bottom: 14px; }
body.fitness-dark .slot-head { display: flex; align-items: baseline; justify-content: space-between; margin-bottom: 7px; }
body.fitness-dark .slot-cal { font-size: 11px; color: var(--noc-faint); }
body.fitness-dark .slot-empty { color: color-mix(in srgb, var(--noc-text) 30%, transparent); }
body.fitness-dark .slot-add-btn { width: 100%; justify-content: flex-start; color: var(--noc-muted); }
body.fitness-dark .entry-main {
  flex: 1; display: flex; align-items: center; gap: 10px; min-width: 0;
  background: none; border: none; padding: 0; font: inherit; color: inherit;
  cursor: pointer; text-align: left; min-height: 44px;
}
body.fitness-dark .slot-chips { display: flex; gap: 7px; flex-wrap: wrap; width: 100%; }
body.fitness-dark .meal-entry-edit form { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; width: 100%; }
body.fitness-dark .meal-entry-edit input[name="grams"] { width: 90px; }
body.fitness-dark .meal-entry-edit select { width: auto; }
```

- [ ] **Step 8: Verify**

Run: `cargo fmt --check && cargo clippy && SQLX_OFFLINE=true cargo test`
Expected: green (including updated legacy meal-entry tests). Browser: entries grouped under Breakfast/Lunch/Dinner/Snack with per-slot kcal; empty Dinner shows "+ Add to dinner"; the log form pre-selects the clock slot; tapping an entry opens the inline edit and Save round-trips grams + slot.

- [ ] **Step 9: Commit**

```bash
git add -A && git commit -m "feat(fitness): meal slots, grouped day view, entry editing (migration 008)"
```

---

### Task 5: Week strip + arbitrary-date page load

**Files:**
- Modify: `src/db.rs` (`get_calories_by_date_range` + test)
- Modify: `src/routes/nutrition.rs` (`week_strip_html`, `?date=` on `fitness_page`, strip rendered by both page and `htmx_day`)
- Modify: `templates/fitness/feed.html` (strip container replaces the date input; arrows stay)
- Modify: `static/style.css`

**Interfaces:**
- Consumes: `Targets` (Task 3), dev-session ritual.
- Produces: `db::get_calories_by_date_range(pool, start: &str, end: &str) -> Vec<(String, f64)>` (date-ascending, only dates with entries); `week_strip_html(week: &[(String, f64)], selected: &str, today: &str, target_cal: f64) -> String` where `week` is exactly 7 `(iso_date, kcal)` pairs Sunday-first; `fitness_page` honors `?date=YYYY-MM-DD`; `htmx_day` returns `<div id="week-strip-inner">…</div>` + day section via HTMX OOB swap — instead, simpler contract: **`htmx_day` returns the day section only, and the strip carries `hx-get` per day targeting `#day-section` plus a small JS hook that repaints the selected bar client-side** (`markSelectedDay(date)`).

- [ ] **Step 1: Write the failing db test**

```rust
    #[tokio::test]
    async fn test_calories_by_date_range() {
        let pool = test_pool().await;
        let item = insert_food_item(&pool, "Rice", "", None, 100.0, 2.0, 20.0, 1.0, 0.0, 0.0, 0.0, 0.0, None, "", "").await;
        insert_meal_entry(&pool, item.id, "2026-07-27", 100.0, "lunch").await.unwrap();
        insert_meal_entry(&pool, item.id, "2026-07-27", 50.0, "dinner").await.unwrap();
        insert_meal_entry(&pool, item.id, "2026-07-29", 200.0, "lunch").await.unwrap();
        insert_meal_entry(&pool, item.id, "2026-08-05", 100.0, "lunch").await.unwrap(); // outside range
        let rows = get_calories_by_date_range(&pool, "2026-07-26", "2026-08-01").await;
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].0, "2026-07-27");
        assert!((rows[0].1 - 150.0).abs() < 0.01);
        assert!((rows[1].1 - 200.0).abs() < 0.01);
    }
```

Run: `SQLX_OFFLINE=true cargo test test_calories_by_date_range` — Expected: FAIL (function missing).

- [ ] **Step 2: Implement the query**

```rust
pub async fn get_calories_by_date_range(pool: &DbPool, start: &str, end: &str) -> Vec<(String, f64)> {
    sqlx::query!(
        r#"SELECT me.date as "date!", SUM(me.grams / 100.0 * fi.calories) as "cal!: f64"
        FROM meal_entries me
        JOIN food_items fi ON fi.id = me.food_item_id
        WHERE me.date >= ? AND me.date <= ?
        GROUP BY me.date
        ORDER BY me.date ASC"#,
        start, end
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|r| (r.date, r.cal))
    .collect()
}
```

`DATABASE_URL=sqlite:portfolio.db cargo sqlx prepare && SQLX_OFFLINE=true cargo test test_calories_by_date_range` — Expected: PASS.

- [ ] **Step 3: Build the strip and wire the handlers**

In `nutrition.rs`:

```rust
/// The Sunday-first week containing `date`, as 7 (iso_date, kcal) pairs.
async fn week_for(pool: &crate::db::DbPool, date: &str) -> Vec<(String, f64)> {
    use chrono::{NaiveDate, Duration, Datelike};
    let d = NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .unwrap_or_else(|_| chrono::Utc::now().date_naive());
    let sunday = d - Duration::days(d.weekday().num_days_from_sunday() as i64);
    let days: Vec<String> = (0..7).map(|i| (sunday + Duration::days(i)).format("%Y-%m-%d").to_string()).collect();
    let cals = crate::db::get_calories_by_date_range(pool, &days[0], &days[6]).await;
    days.into_iter()
        .map(|day| {
            let cal = cals.iter().find(|(d2, _)| *d2 == day).map(|(_, c)| *c).unwrap_or(0.0);
            (day, cal)
        })
        .collect()
}

fn week_strip_html(week: &[(String, f64)], selected: &str, today: &str, target_cal: f64) -> String {
    const LETTERS: [&str; 7] = ["S", "M", "T", "W", "T", "F", "S"];
    let cols: String = week.iter().enumerate().map(|(i, (day, cal))| {
        let is_selected = day.as_str() == selected;
        let is_future = day.as_str() > today;
        let pct = if target_cal > 0.0 { ((cal / target_cal) * 100.0).clamp(0.0, 112.0) } else { 0.0 };
        let (cell_cls, fill) = if is_future {
            ("day-cell future", String::new())
        } else if is_selected {
            ("day-cell selected", format!(r#"<div class="day-fill accent" style="height:{pct:.0}%"></div>"#))
        } else {
            ("day-cell", format!(r#"<div class="day-fill" style="height:{pct:.0}%"></div>"#))
        };
        let letter_cls = if is_selected { "day-letter selected" } else if is_future { "day-letter future" } else { "day-letter" };
        format!(
            r##"<button type="button" class="day-col" data-date="{day}" onclick="loadDay('{day}')" aria-label="{day}">
  <span class="{letter_cls}">{letter}</span>
  <div class="{cell_cls}">{fill}</div>
</button>"##,
            day = day, letter = LETTERS[i], letter_cls = letter_cls, cell_cls = cell_cls, fill = fill
        )
    }).collect();
    format!(r#"<div class="week-strip" id="week-strip">{}</div>"#, cols)
}
```

`fitness_page` gains `Query(params): Query<HashMap<String, String>>` and uses `params.get("date")` (validated `NaiveDate::parse_from_str`, falling back to today) instead of always-today. It renders the strip into a new template field `week_strip_html: String` (add to `FitnessTemplate`) and passes the selected date as `today` field's replacement — rename the template field `today` to `date` and add `today: String` (actual today) for the JS. In `templates/fitness/feed.html`, replace the date `<input>` with `{{ week_strip_html|safe }}` between the two arrow buttons, and set:

```js
window.TODAY = "{{ today }}";
window.currentDate = "{{ date }}";
```

`loadDay(date)` keeps working via `htmx.ajax` on `#day-section`; add a client-side repaint so the strip tracks selection without a round-trip:

```js
function markSelectedDay(date) {
  document.querySelectorAll('#week-strip .day-col').forEach(col => {
    const sel = col.dataset.date === date;
    col.querySelector('.day-letter').classList.toggle('selected', sel);
    col.querySelector('.day-cell').classList.toggle('selected', sel);
  });
}
```

called at the top of `loadDay`. When the selected day moves outside the rendered week (arrows crossing a Sunday), `loadDay` falls back to a full navigation: `if (![...document.querySelectorAll('#week-strip .day-col')].some(c => c.dataset.date === date)) { location.href = '/fitness?date=' + date; return; }`.

- [ ] **Step 4: CSS**

```css
/* week strip (Task 5) */
body.fitness-dark .week-strip { flex: 1; display: grid; grid-template-columns: repeat(7, 1fr); gap: 6px; }
body.fitness-dark .day-col { background: none; border: none; padding: 0; cursor: pointer; display: flex; flex-direction: column; align-items: center; gap: 6px; min-height: 44px; }
body.fitness-dark .day-letter { font-size: 10px; color: var(--noc-faint); }
body.fitness-dark .day-letter.selected { color: var(--noc-text); font-weight: 600; }
body.fitness-dark .day-letter.future { color: color-mix(in srgb, var(--noc-text) 25%, transparent); }
body.fitness-dark .day-cell { width: 100%; height: 34px; background: var(--noc-surface); border-radius: var(--noc-radius-sm); display: flex; align-items: flex-end; overflow: hidden; }
body.fitness-dark .day-cell.selected { background: var(--noc-accent-900); box-shadow: 0 0 0 1px var(--noc-accent); }
body.fitness-dark .day-cell.future { background: var(--noc-surface-2); }
body.fitness-dark .day-fill { width: 100%; background: var(--noc-n800); border-radius: var(--noc-radius-sm); }
body.fitness-dark .day-fill.accent { background: var(--noc-accent); }
```

- [ ] **Step 5: Verify**

Run: `cargo fmt --check && cargo clippy && SQLX_OFFLINE=true cargo test`
Expected: green. Browser: strip shows the week with today outlined in accent, taps load that day, arrows still step (and cross weeks via full navigation), `/fitness?date=2026-07-30` opens that day directly.

- [ ] **Step 6: Commit**

```bash
git add -A && git commit -m "feat(fitness): week strip and date-addressable day view"
```

---

### Task 6: Copy yesterday + sticky bottom action bar

**Files:**
- Modify: `src/db.rs` (`copy_day_entries` + test)
- Modify: `src/routes/nutrition.rs` (`POST /fitness/copy-day`)
- Modify: `templates/fitness/feed.html` (bottom bar)
- Modify: `static/style.css`

**Interfaces:**
- Consumes: `insert_meal_entry(pool, food_item_id, date, grams, slot)` from Task 4; `defaultSlot()` etc. from Task 4.
- Produces: `db::copy_day_entries(pool, from_date: &str, to_date: &str) -> u64` (rows copied, slots preserved); route `POST /fitness/copy-day` (form fields `date`; copies from the day before `date`), returns the day section; bottom bar element `#fitness-actionbar` with buttons `#bar-scan`, `#bar-search`, `#bar-copy` (Task 7 rewires `#bar-scan`/`#bar-search` to the add sheet — this task points them at the existing scanner and the library search).

- [ ] **Step 1: Write the failing db test**

```rust
    #[tokio::test]
    async fn test_copy_day_entries() {
        let pool = test_pool().await;
        let item = insert_food_item(&pool, "Oats", "", None, 379.0, 13.2, 60.1, 6.5, 0.0, 0.0, 0.0, 0.0, None, "", "").await;
        insert_meal_entry(&pool, item.id, "2026-07-31", 80.0, "breakfast").await.unwrap();
        insert_meal_entry(&pool, item.id, "2026-07-31", 120.0, "lunch").await.unwrap();
        let copied = copy_day_entries(&pool, "2026-07-31", "2026-08-01").await;
        assert_eq!(copied, 2);
        let entries = get_meal_entries_for_date(&pool, "2026-08-01").await;
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].slot, "breakfast");
        assert_eq!(entries[1].grams, 120.0);
    }
```

Run: `SQLX_OFFLINE=true cargo test test_copy_day_entries` — Expected: FAIL.

- [ ] **Step 2: Implement**

```rust
pub async fn copy_day_entries(pool: &DbPool, from_date: &str, to_date: &str) -> u64 {
    sqlx::query!(
        "INSERT INTO meal_entries (food_item_id, date, grams, slot)
         SELECT food_item_id, ?, grams, slot FROM meal_entries WHERE date = ?",
        to_date, from_date
    )
    .execute(pool)
    .await
    .map(|r| r.rows_affected())
    .unwrap_or(0)
}
```

Handler + route in `nutrition.rs`:

```rust
async fn copy_day_handler(
    AuthSession(_): AuthSession,
    State(state): State<Arc<AppState>>,
    axum::Form(form): axum::Form<HashMap<String, String>>,
) -> impl IntoResponse {
    let date = form.get("date").cloned().unwrap_or_else(|| chrono::Utc::now().format("%Y-%m-%d").to_string());
    let yesterday = chrono::NaiveDate::parse_from_str(&date, "%Y-%m-%d")
        .map(|d| (d - chrono::Duration::days(1)).format("%Y-%m-%d").to_string())
        .unwrap_or_default();
    if !yesterday.is_empty() {
        crate::db::copy_day_entries(&state.pool, &yesterday, &date).await;
    }
    let targets = crate::db::get_targets(&state.pool).await;
    let entries = crate::db::get_meal_entries_for_date(&state.pool, &date).await;
    let food_items = crate::db::get_food_items(&state.pool).await;
    Html(day_section_html(&entries, &date, &food_items, &targets, true))
}
```

```rust
        .route("/fitness/copy-day", post(copy_day_handler))
```

`DATABASE_URL=sqlite:portfolio.db cargo sqlx prepare && SQLX_OFFLINE=true cargo test test_copy_day_entries` — Expected: PASS.

- [ ] **Step 3: Bottom bar markup** (end of `.fitness-page` in `feed.html`)

```html
  <div id="fitness-actionbar">
    <button id="bar-scan" class="noc-btn noc-btn-primary" onclick="startBarcodeScanner('add-food-form')">
      <svg width="18" height="14" viewBox="0 0 18 14" fill="none" stroke="currentColor" stroke-width="1.5"><path d="M1 1v12M4.5 1v12M7.5 1v12M11 1v12M14 1v12M17 1v12"></path></svg>
      Scan
    </button>
    <button id="bar-search" class="noc-btn noc-btn-secondary" onclick="document.getElementById('library-search').focus()">
      <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.6"><circle cx="7" cy="7" r="5"></circle><path d="M11 11l4 4"></path></svg>
      Search
    </button>
    <button id="bar-copy" class="noc-btn noc-btn-secondary noc-btn-icon" title="Copy yesterday"
            hx-post="/fitness/copy-day" hx-vals='js:{date: window.currentDate}' hx-target="#day-section" hx-swap="innerHTML">
      <svg width="17" height="17" viewBox="0 0 17 17" fill="none" stroke="currentColor" stroke-width="1.5"><rect x="2" y="2" width="9" height="11" rx="2"></rect><path d="M6 15h7a2 2 0 0 0 2-2V6"></path></svg>
    </button>
  </div>
```

- [ ] **Step 4: CSS**

```css
/* bottom action bar (Task 6) */
body.fitness-dark #fitness-actionbar {
  position: fixed; bottom: 0; left: 0; right: 0; z-index: 40;
  display: flex; gap: 10px; padding: 12px 16px calc(12px + env(safe-area-inset-bottom));
  max-width: 640px; margin: 0 auto;
  background: linear-gradient(to top, var(--noc-bg) 60%, color-mix(in srgb, var(--noc-bg) 0%, transparent));
}
body.fitness-dark #fitness-actionbar .noc-btn { flex: 1; min-height: 50px; font-size: 15px; }
body.fitness-dark #fitness-actionbar .noc-btn-icon { flex: none; width: 50px; }
```

- [ ] **Step 5: Verify + commit**

Run: `cargo fmt --check && cargo clippy && SQLX_OFFLINE=true cargo test` — Expected: green. Browser: bar floats over the page bottom; Copy yesterday duplicates the previous day's entries into the visible day with slots intact.

```bash
git add -A && git commit -m "feat(fitness): copy-yesterday and sticky bottom action bar"
```

---

### Task 7: Scan-first add sheet (recents, search, one-tap portions)

**Files:**
- Modify: `src/db.rs` (`get_recent_foods`, `get_food_item_by_barcode` + tests)
- Modify: `src/models.rs` (`RecentFood`)
- Modify: `src/routes/nutrition.rs` (`match_card_html`, sheet fragment endpoints)
- Modify: `templates/fitness/feed.html` (sheet overlay markup + JS)
- Modify: `static/barcode.js` (route detections into the sheet)
- Modify: `static/style.css`

**Interfaces:**
- Consumes: `POST /api/nutrition/entries` with `slot` (Task 4); `defaultSlot()`/`paintSlotChips`/`setSlot` (Task 4); `#bar-scan`/`#bar-search` (Task 6).
- Produces: `pub struct RecentFood { pub food_item_id: i64, pub name: String, pub last_grams: f64, pub last_slot: String }` in models; `db::get_recent_foods(pool, limit: i64) -> Vec<RecentFood>`; `db::get_food_item_by_barcode(pool, barcode: &str) -> Option<FoodItem>`; routes `GET /fitness/htmx/recent`, `GET /fitness/htmx/food-search?q=`, `GET /fitness/htmx/match-card/{food_item_id}` (returns the log card); JS `openAddSheet(tab)` / `closeAddSheet()` globals; `window.onBarcodeMatch(code)` hook consumed by `barcode.js`.

- [ ] **Step 1: Write the failing db tests**

```rust
    #[tokio::test]
    async fn test_recent_foods_dedup_and_order() {
        let pool = test_pool().await;
        let a = insert_food_item(&pool, "Skyr", "", None, 63.0, 11.0, 4.0, 0.2, 0.0, 0.0, 0.0, 0.0, None, "", "").await;
        let b = insert_food_item(&pool, "Oats", "", None, 379.0, 13.2, 60.1, 6.5, 0.0, 0.0, 0.0, 0.0, None, "", "").await;
        insert_meal_entry(&pool, a.id, "2026-07-30", 250.0, "breakfast").await.unwrap();
        insert_meal_entry(&pool, b.id, "2026-07-31", 80.0, "breakfast").await.unwrap();
        insert_meal_entry(&pool, a.id, "2026-08-01", 300.0, "snack").await.unwrap();
        let recent = get_recent_foods(&pool, 8).await;
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].name, "Skyr");        // most recently logged first
        assert_eq!(recent[0].last_grams, 300.0);   // grams of the latest log
        assert_eq!(recent[0].last_slot, "snack");
    }

    #[tokio::test]
    async fn test_get_food_item_by_barcode() {
        let pool = test_pool().await;
        insert_food_item(&pool, "Bar", "Barebells", Some("5060123456789"), 200.0, 20.0, 16.0, 8.0, 0.0, 0.0, 0.0, 0.0, Some(55.0), "", "").await;
        assert!(get_food_item_by_barcode(&pool, "5060123456789").await.is_some());
        assert!(get_food_item_by_barcode(&pool, "0000000000000").await.is_none());
    }
```

Run: `SQLX_OFFLINE=true cargo test test_recent_foods_dedup_and_order test_get_food_item_by_barcode` — Expected: FAIL.

- [ ] **Step 2: Implement models + db**

`src/models.rs`:

```rust
#[derive(Debug, Clone)]
pub struct RecentFood {
    pub food_item_id: i64,
    pub name: String,
    pub last_grams: f64,
    pub last_slot: String,
}
```

`src/db.rs` (SQLite: `MAX(created_at)` in the GROUP BY row makes the other selected `me.*` columns come from that same row — documented SQLite behavior for bare columns with MAX):

```rust
pub async fn get_recent_foods(pool: &DbPool, limit: i64) -> Vec<crate::models::RecentFood> {
    sqlx::query!(
        r#"SELECT me.food_item_id as "food_item_id!", fi.name as "name!",
                  me.grams as "last_grams!: f64", me.slot as "last_slot!",
                  MAX(me.created_at || '-' || printf('%012d', me.id)) as "latest!: String"
        FROM meal_entries me
        JOIN food_items fi ON fi.id = me.food_item_id
        GROUP BY me.food_item_id
        ORDER BY 5 DESC
        LIMIT ?"#,
        limit
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|r| crate::models::RecentFood {
        food_item_id: r.food_item_id,
        name: r.name,
        last_grams: r.last_grams,
        last_slot: r.last_slot,
    })
    .collect()
}

pub async fn get_food_item_by_barcode(pool: &DbPool, barcode: &str) -> Option<FoodItem> {
    sqlx::query_as!(FoodItem,
        "SELECT id, name, brand, barcode, calories, protein, carbs, fat, fiber, sugar, sodium, saturated_fat, package_size, custom_portions, image_url, created_at FROM food_items WHERE barcode = ?",
        barcode
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
}
```

`DATABASE_URL=sqlite:portfolio.db cargo sqlx prepare && SQLX_OFFLINE=true cargo test` — Expected: PASS.

- [ ] **Step 3: Match card + fragment endpoints** (`nutrition.rs`)

```rust
/// The log card shown when a scan / search / recent tap resolves to a food item.
/// Portion buttons: package fractions (full/half) and each custom portion; grams input as fallback.
fn match_card_html(item: &crate::models::FoodItem, kicker: &str) -> String {
    let mut portions: Vec<(String, f64)> = Vec::new();
    if let Some(pkg) = item.package_size {
        portions.push((format!("{} g", fmt_nutrient(pkg)), pkg));
        portions.push((format!("Half {} g", fmt_nutrient(pkg * 0.5)), pkg * 0.5));
    }
    for s in item.custom_portions.split(',') {
        if let Ok(g) = s.trim().parse::<f64>() {
            if g > 0.0 { portions.push((format!("{} g", fmt_nutrient(g)), g)); }
        }
    }
    portions.truncate(3);
    let portion_btns: String = portions.iter().enumerate().map(|(i, (label, g))| format!(
        r##"<button type="button" class="noc-btn {cls} portion-btn" data-grams="{g}" onclick="pickPortion(this)">{label}</button>"##,
        cls = if i == 0 { "noc-btn-primary" } else { "noc-btn-secondary" },
        g = g, label = label
    )).collect();
    let default_grams = portions.first().map(|(_, g)| *g).unwrap_or(100.0);
    let brand = if item.brand.is_empty() { String::new() } else { format!("{} · ", html_escape(&item.brand)) };
    format!(
        r##"<div class="match-card noc-card" id="match-card">
  <div class="match-head">
    <div class="match-title">{name}</div>
    <div class="match-sub">{brand}{cal} cal · P {p} · C {c} · F {f} / 100 g</div>
    <span class="noc-kicker">{kicker}</span>
  </div>
  <form hx-post="/api/nutrition/entries" hx-target="#day-section" hx-swap="innerHTML"
        hx-on::after-request="closeAddSheet()">
    <input type="hidden" name="date" value="">
    <input type="hidden" name="food_item_id" value="{id}">
    <input type="hidden" name="slot" value="other">
    <div class="noc-kicker">Portion</div>
    <div class="portion-row">{portion_btns}
      <input class="noc-input portion-grams" type="number" name="grams" value="{default_grams}" min="1" max="5000" step="0.1" required>
    </div>
    <div class="noc-kicker">Meal</div>
    <div class="slot-chips" data-role="slot-chips">
      <button type="button" class="noc-tag noc-tag-outline" data-slot="breakfast" onclick="setSlot(this)">Breakfast</button>
      <button type="button" class="noc-tag noc-tag-outline" data-slot="lunch" onclick="setSlot(this)">Lunch</button>
      <button type="button" class="noc-tag noc-tag-outline" data-slot="dinner" onclick="setSlot(this)">Dinner</button>
      <button type="button" class="noc-tag noc-tag-outline" data-slot="snack" onclick="setSlot(this)">Snack</button>
    </div>
    <button type="submit" class="noc-btn noc-btn-primary match-log-btn">Log it</button>
  </form>
</div>"##,
        name = html_escape(&item.name), brand = brand,
        cal = fmt_nutrient(item.calories), p = fmt_nutrient(item.protein),
        c = fmt_nutrient(item.carbs), f = fmt_nutrient(item.fat),
        kicker = html_escape(kicker), id = item.id,
        portion_btns = portion_btns, default_grams = fmt_nutrient(default_grams)
    )
}

async fn match_card(
    AuthSession(_): AuthSession,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    match crate::db::get_food_item(&state.pool, id).await {
        Some(item) => Html(match_card_html(&item, "From library")).into_response(),
        None => (StatusCode::NOT_FOUND, Html(String::new())).into_response(),
    }
}

async fn recent_chips(
    AuthSession(_): AuthSession,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let recents = crate::db::get_recent_foods(&state.pool, 8).await;
    let chips: String = recents.iter().map(|r| format!(
        r##"<button type="button" class="noc-btn noc-btn-secondary recent-chip"
             hx-get="/fitness/htmx/match-card/{id}" hx-target="#sheet-result" hx-swap="innerHTML">{name} {grams} g</button>"##,
        id = r.food_item_id, name = html_escape(&r.name), grams = fmt_nutrient(r.last_grams)
    )).collect();
    Html(if chips.is_empty() { "<p class=\"sheet-hint\">Nothing logged yet.</p>".to_string() } else { chips })
}

async fn food_search(
    AuthSession(_): AuthSession,
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let q = params.get("q").cloned().unwrap_or_default();
    if q.trim().is_empty() { return Html(String::new()); }
    let items = crate::db::search_food_items(&state.pool, q.trim()).await;
    let rows: String = items.iter().map(|i| format!(
        r##"<button type="button" class="search-row"
             hx-get="/fitness/htmx/match-card/{id}" hx-target="#sheet-result" hx-swap="innerHTML">
      <span class="search-name">{name}</span>
      <span class="search-macros">{cal} cal / 100 g</span>
    </button>"##,
        id = i.id, name = html_escape(&i.name), cal = fmt_nutrient(i.calories)
    )).collect();
    Html(rows)
}

async fn barcode_match(
    AuthSession(_): AuthSession,
    State(state): State<Arc<AppState>>,
    Path(code): Path<String>,
) -> impl IntoResponse {
    match crate::db::get_food_item_by_barcode(&state.pool, &code).await {
        Some(item) => Html(match_card_html(&item, &format!("Matched · {}", html_escape(&code)))).into_response(),
        None => (StatusCode::NOT_FOUND, Html(String::new())).into_response(),
    }
}
```

Routes:

```rust
        .route("/fitness/htmx/recent", get(recent_chips))
        .route("/fitness/htmx/food-search", get(food_search))
        .route("/fitness/htmx/match-card/{id}", get(match_card))
        .route("/fitness/htmx/barcode-match/{code}", get(barcode_match))
```

- [ ] **Step 4: Sheet overlay markup + JS** (`feed.html`, after the action bar)

```html
  <div id="add-sheet" hidden>
    <div class="sheet-head">
      <span class="noc-kicker">Add food</span>
      <button class="noc-btn noc-btn-ghost noc-btn-icon" onclick="closeAddSheet()" aria-label="Close">✕</button>
    </div>
    <div class="sheet-tabs">
      <button class="noc-tag noc-tag-accent" data-tab="scan" onclick="sheetTab('scan')">Scan</button>
      <button class="noc-tag noc-tag-outline" data-tab="recent" onclick="sheetTab('recent')">Recent</button>
      <button class="noc-tag noc-tag-outline" data-tab="search" onclick="sheetTab('search')">Search</button>
    </div>
    <div id="sheet-scan" class="sheet-pane">
      <div id="barcode-scanner-frame">
        <video id="barcode-video" autoplay playsinline></video>
        <p id="scan-status">Hold the barcode inside the frame</p>
      </div>
      <div id="manual-barcode">
        <input class="noc-input" type="text" id="manual-barcode-input" placeholder="Enter barcode number" inputmode="numeric">
        <button class="noc-btn noc-btn-secondary" onclick="lookupManualBarcode()">Look up</button>
      </div>
    </div>
    <div id="sheet-recent" class="sheet-pane" hidden
         hx-get="/fitness/htmx/recent" hx-trigger="sheet-open from:body" hx-target="find .chips" hx-swap="innerHTML">
      <div class="noc-kicker">Log again</div>
      <div class="chips"></div>
    </div>
    <div id="sheet-search" class="sheet-pane" hidden>
      <input class="noc-input" type="search" id="sheet-search-input" placeholder="Search your library…"
             hx-get="/fitness/htmx/food-search" hx-trigger="input changed delay:250ms" hx-target="#sheet-search-results" hx-swap="innerHTML" name="q">
      <div id="sheet-search-results"></div>
    </div>
    <div id="sheet-result"></div>
  </div>
```

JS (same script block; note the existing scanner element IDs move into the sheet — the old `#barcode-scanner` block outside is deleted):

```js
function openAddSheet(tab) {
  const sheet = document.getElementById('add-sheet');
  sheet.hidden = false;
  document.body.classList.add('sheet-open');
  sheetTab(tab || 'scan');
  document.body.dispatchEvent(new Event('sheet-open'));
  if ((tab || 'scan') === 'scan') startBarcodeScanner();
}

function closeAddSheet() {
  stopBarcodeScanner();
  document.getElementById('add-sheet').hidden = true;
  document.getElementById('sheet-result').innerHTML = '';
  document.body.classList.remove('sheet-open');
}

function sheetTab(tab) {
  ['scan', 'recent', 'search'].forEach(t => {
    document.getElementById('sheet-' + t).hidden = t !== tab;
    const btn = document.querySelector('#add-sheet [data-tab="' + t + '"]');
    btn.classList.toggle('noc-tag-accent', t === tab);
    btn.classList.toggle('noc-tag-outline', t !== tab);
  });
  if (tab === 'scan') startBarcodeScanner(); else stopBarcodeScanner();
  if (tab === 'search') document.getElementById('sheet-search-input').focus();
}

function pickPortion(btn) {
  const card = btn.closest('form');
  card.querySelector('.portion-grams').value = btn.dataset.grams;
  card.querySelectorAll('.portion-btn').forEach(b => {
    b.classList.toggle('noc-btn-primary', b === btn);
    b.classList.toggle('noc-btn-secondary', b !== btn);
  });
}

// Fill date + clock slot into a freshly swapped match card
function initMatchCard() {
  const card = document.querySelector('#sheet-result form');
  if (!card || card.dataset.init) return;
  card.dataset.init = '1';
  card.querySelector('[name="date"]').value = window.currentDate;
  card.querySelector('[name="slot"]').value = defaultSlot();
  paintSlotChips(card);
}
document.body.addEventListener('htmx:afterSwap', initMatchCard);

// Called by barcode.js on every decoded barcode
window.onBarcodeMatch = function (code) {
  htmx.ajax('GET', '/fitness/htmx/barcode-match/' + encodeURIComponent(code), {
    target: '#sheet-result', swap: 'innerHTML'
  }).then(() => {
    if (!document.querySelector('#sheet-result form')) {
      // unknown product: fall back to the existing OpenFoodFacts add-food flow
      closeAddSheet();
      openOffLookup(code); // defined in barcode.js
    }
  });
};
```

Rewire the Task 6 bar buttons:

Only the `onclick` attributes change — the buttons' inner SVG + label markup from Task 6 stays exactly as written there:

```html
    <button id="bar-scan" class="noc-btn noc-btn-primary" onclick="openAddSheet('scan')">
    <button id="bar-search" class="noc-btn noc-btn-secondary" onclick="openAddSheet('search')">
```

- [ ] **Step 5: Rework `static/barcode.js`**

Read the file first. Keep its `BarcodeDetector` loop, camera handling and OpenFoodFacts fetch intact, with these contract changes:

1. `startBarcodeScanner()` takes no argument and targets the sheet's `#barcode-video` / `#scan-status` elements.
2. On a decoded barcode, instead of writing to the add-food form directly, call `window.onBarcodeMatch(code)` and stop scanning.
3. Extract the current "prefill the add-food form from OpenFoodFacts" logic into an exported `openOffLookup(code)` that un-hides the library add-food form, prefills name/brand/barcode/macros/image_url from the OFF response, and scrolls to it.
4. `lookupManualBarcode()` reads `#manual-barcode-input` and calls `window.onBarcodeMatch(code)`.
5. If `BarcodeDetector` is unavailable, the scan pane shows the manual input (it is always visible in the pane — just skip the camera start and set `#scan-status` text to "Camera scanning not supported here — enter the code".)

- [ ] **Step 6: CSS**

```css
/* add sheet (Task 7) */
body.fitness-dark #add-sheet {
  position: fixed; inset: 0; z-index: 50; overflow-y: auto;
  background: var(--noc-bg); padding: 16px 16px 32px;
  max-width: 640px; margin: 0 auto;
}
body.fitness-dark.sheet-open { overflow: hidden; }
body.fitness-dark .sheet-head { display: flex; align-items: center; justify-content: space-between; margin-bottom: 10px; }
body.fitness-dark .sheet-tabs { display: flex; gap: 7px; flex-wrap: wrap; margin-bottom: 14px; }
body.fitness-dark .sheet-pane { margin-bottom: 14px; }
body.fitness-dark #barcode-scanner-frame {
  border-radius: var(--noc-radius-lg); overflow: hidden; background: #0b0c14;
  box-shadow: 0 0 0 1px var(--noc-n900); text-align: center; padding-bottom: 10px;
}
body.fitness-dark #barcode-scanner-frame video { width: 100%; max-height: 260px; object-fit: cover; display: block; }
body.fitness-dark #manual-barcode { display: flex; gap: 8px; margin-top: 10px; }
body.fitness-dark .sheet-hint { color: var(--noc-faint); font-size: 13px; }
body.fitness-dark #sheet-recent .chips { display: flex; gap: 8px; flex-wrap: wrap; margin-top: 8px; }
body.fitness-dark .search-row {
  display: flex; justify-content: space-between; align-items: center; gap: 10px; width: 100%;
  background: var(--noc-surface); border: none; border-radius: var(--noc-radius-md);
  box-shadow: var(--noc-shadow-card); padding: 12px 14px; margin-top: 8px;
  font: inherit; color: inherit; cursor: pointer; min-height: 44px;
}
body.fitness-dark .search-row:hover { background: color-mix(in srgb, var(--noc-text) 7%, var(--noc-surface)); }
body.fitness-dark .search-macros { color: var(--noc-muted); font-size: 12px; white-space: nowrap; }
body.fitness-dark .match-card { margin-top: 14px; box-shadow: var(--noc-shadow-pop); }
body.fitness-dark .match-title { font-size: 17px; font-weight: 500; letter-spacing: -.01em; }
body.fitness-dark .match-sub { font-size: 12px; color: var(--noc-muted); margin: 3px 0 6px; }
body.fitness-dark .portion-row { display: flex; gap: 7px; flex-wrap: wrap; margin: 8px 0 12px; }
body.fitness-dark .portion-row .portion-btn { flex: 1; }
body.fitness-dark .portion-grams { width: 96px; flex: none; }
body.fitness-dark .match-log-btn { width: 100%; min-height: 50px; font-size: 15px; margin-top: 14px; }
```

- [ ] **Step 7: Verify + commit**

Run: `cargo fmt --check && cargo clippy && SQLX_OFFLINE=true cargo test` — Expected: green (new db tests pass). Browser: Scan opens the sheet on the camera tab; manual code entry of a known barcode shows the match card; a portion tap + Log it lands the entry in the clock-inferred slot and closes the sheet; Recent chips log-again flow works; Search tab finds library items. Compare with mockup screens 1b and 2a–2d.

```bash
git add -A && git commit -m "feat(fitness): scan-first add sheet with recents, search and one-tap portions"
```

---

### Task 8: Library grouping, favourites, food detail sheet (migration 010)

**Files:**
- Create: `migrations/010_food_meta.sql`
- Modify: `src/db.rs` (register migration; extend FoodItem selects + `update_food_item`; `toggle_food_favourite`, `get_item_log_history`; tests)
- Modify: `src/models.rs` (`FoodItem` gains `category`, `is_favourite`, `default_portion_g`)
- Modify: `src/routes/nutrition.rs` (grouped library, filter chips, detail form, favourite route)
- Modify: `templates/fitness/feed.html` (chips row + client-side filter JS)
- Modify: `static/style.css`

**Interfaces:**
- Consumes: `.noc-*` primitives; `match_card_html` (favourites tab reuses `GET /fitness/htmx/match-card/{id}`).
- Produces: `FoodItem` gains `pub category: String`, `pub is_favourite: i64`, `pub default_portion_g: Option<f64>` — **every `query_as!(FoodItem, …)` SELECT in db.rs must list the new columns**; `update_food_item` gains trailing params `category: &str, is_favourite: bool, default_portion_g: Option<f64>`; `insert_food_item` keeps its arity (new items start uncategorised — category/favourite/default portion are set via the detail form; decision recorded here); `db::toggle_food_favourite(pool, id: i64)`; `db::get_item_log_history(pool, id: i64, start: &str, end: &str) -> Vec<(String, f64)>` (date, grams per day); route `POST /api/nutrition/food-items/{id}/favourite` returns the refreshed library; `library_list_html(items: &[FoodItem], recent_ids: &std::collections::HashSet<i64>, is_admin: bool) -> String` (**signature change**); a `Favourites` tab in the add sheet (`GET /fitness/htmx/favourites`).

- [ ] **Step 1: Migration + apply**

`migrations/010_food_meta.sql`:

```sql
ALTER TABLE food_items ADD COLUMN category TEXT NOT NULL DEFAULT '';
ALTER TABLE food_items ADD COLUMN is_favourite INTEGER NOT NULL DEFAULT 0;
ALTER TABLE food_items ADD COLUMN default_portion_g REAL;
```

Register after 009 in `run_migrations` (same `let _ =` pattern), apply with the python3 one-liner.

- [ ] **Step 2: Failing db tests**

```rust
    #[tokio::test]
    async fn test_favourite_and_category_roundtrip() {
        let pool = test_pool().await;
        let item = insert_food_item(&pool, "Skyr", "Arla", None, 63.0, 11.0, 4.0, 0.2, 0.0, 4.0, 45.0, 0.1, Some(450.0), "", "").await;
        assert_eq!(item.category, "");
        assert_eq!(item.is_favourite, 0);
        toggle_food_favourite(&pool, item.id).await;
        update_food_item(&pool, item.id, "Skyr", "Arla", None, 63.0, 11.0, 4.0, 0.2, 0.0, 4.0, 45.0, 0.1, Some(450.0), "", "", "Dairy & eggs", true, Some(170.0)).await;
        let item = get_food_item(&pool, item.id).await.unwrap();
        assert_eq!(item.category, "Dairy & eggs");
        assert_eq!(item.is_favourite, 1);
        assert_eq!(item.default_portion_g, Some(170.0));
        toggle_food_favourite(&pool, item.id).await;
        assert_eq!(get_food_item(&pool, item.id).await.unwrap().is_favourite, 0);
    }

    #[tokio::test]
    async fn test_item_log_history() {
        let pool = test_pool().await;
        let item = insert_food_item(&pool, "Oats", "", None, 379.0, 13.0, 60.0, 6.5, 0.0, 0.0, 0.0, 0.0, None, "", "").await;
        insert_meal_entry(&pool, item.id, "2026-07-30", 80.0, "breakfast").await.unwrap();
        insert_meal_entry(&pool, item.id, "2026-07-30", 40.0, "snack").await.unwrap();
        insert_meal_entry(&pool, item.id, "2026-07-31", 80.0, "breakfast").await.unwrap();
        let hist = get_item_log_history(&pool, item.id, "2026-07-18", "2026-07-31").await;
        assert_eq!(hist.len(), 2);
        assert_eq!(hist[0], ("2026-07-30".to_string(), 120.0));
    }
```

Run: expected FAIL (missing fields/functions).

- [ ] **Step 3: Implement**

`src/models.rs`, `FoodItem` gains:

```rust
    pub category: String,
    pub is_favourite: i64,
    pub default_portion_g: Option<f64>,
```

`src/db.rs` — append `, category, is_favourite, default_portion_g` to the column list of **all four** `query_as!(FoodItem, …)` calls (`get_food_items`, `search_food_items`, the post-insert fetch in `insert_food_item`, `get_food_item`, `get_food_item_by_barcode`). Extend `update_food_item`:

```rust
pub async fn update_food_item(
    pool: &DbPool, id: i64, name: &str, brand: &str, barcode: Option<&str>,
    calories: f64, protein: f64, carbs: f64, fat: f64, fiber: f64, sugar: f64,
    sodium: f64, saturated_fat: f64, package_size: Option<f64>,
    custom_portions: &str, image_url: &str,
    category: &str, is_favourite: bool, default_portion_g: Option<f64>,
) {
    let fav = if is_favourite { 1i64 } else { 0i64 };
    sqlx::query!(
        "UPDATE food_items SET name = ?, brand = ?, barcode = ?, calories = ?, protein = ?, carbs = ?, fat = ?, fiber = ?, sugar = ?, sodium = ?, saturated_fat = ?, package_size = ?, custom_portions = ?, image_url = ?, category = ?, is_favourite = ?, default_portion_g = ? WHERE id = ?",
        name, brand, barcode, calories, protein, carbs, fat, fiber, sugar, sodium, saturated_fat, package_size, custom_portions, image_url, category, fav, default_portion_g, id
    )
    .execute(pool)
    .await
    .ok();
}

pub async fn toggle_food_favourite(pool: &DbPool, id: i64) {
    sqlx::query!("UPDATE food_items SET is_favourite = 1 - is_favourite WHERE id = ?", id)
        .execute(pool)
        .await
        .ok();
}

pub async fn get_item_log_history(pool: &DbPool, id: i64, start: &str, end: &str) -> Vec<(String, f64)> {
    sqlx::query!(
        r#"SELECT date as "date!", SUM(grams) as "grams!: f64" FROM meal_entries
        WHERE food_item_id = ? AND date >= ? AND date <= ?
        GROUP BY date ORDER BY date ASC"#,
        id, start, end
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|r| (r.date, r.grams))
    .collect()
}
```

The existing caller of `update_food_item` (`update_food_item_handler`) parses the three new form fields: `category` (text), `is_favourite` (checkbox → `form field "is_favourite" present`), `default_portion_g` (number, `> 0.0` else None). `cargo sqlx prepare`, tests → PASS.

- [ ] **Step 4: Grouped library + chips + detail form**

`library_list_html` groups by `category` (empty → `"Uncategorised"`, sorted alphabetically, Uncategorised last) and stamps filter data attributes; `food_item_card_html` becomes the mockup-1c card. Replace both functions:

```rust
pub fn food_item_card_html(item: &crate::models::FoodItem, is_recent: bool, is_admin: bool) -> String {
    let img_html = if item.image_url.is_empty() {
        r#"<div class="food-thumb food-thumb-empty"></div>"#.to_string()
    } else {
        format!("<img src=\"{}\" alt=\"{}\" class=\"food-thumb\" loading=\"lazy\">",
            html_escape(&item.image_url), html_escape(&item.name))
    };
    let brand_html = if item.brand.is_empty() { String::new() }
        else { format!("<span class=\"food-brand\">{}</span>", html_escape(&item.brand)) };
    let pkg_badge = if let Some(pkg) = item.package_size {
        format!("<span class=\"noc-tag noc-tag-neutral food-pkg-badge\">{} g</span>", fmt_nutrient(pkg))
    } else { String::new() };
    let fav_btn = format!(
        "<button class=\"fav-btn{}\" hx-post=\"/api/nutrition/food-items/{}/favourite\" \
         hx-target=\"#food-library\" hx-swap=\"innerHTML\" aria-label=\"Toggle favourite\">★</button>",
        if item.is_favourite != 0 { " is-fav" } else { "" }, item.id
    );
    let admin_btns = if is_admin {
        format!(
            "<div class=\"food-admin-btns\">{fav}\
             <button class=\"food-edit-btn\" hx-get=\"/api/nutrition/food-items/{id}/edit\" \
             hx-target=\"#food-item-{id}\" hx-swap=\"outerHTML\">Edit</button>\
             <button class=\"food-delete-btn\" hx-delete=\"/api/nutrition/food-items/{id}\" \
             hx-target=\"#food-library\" hx-swap=\"innerHTML\" \
             hx-confirm=\"Delete this food item?\">×</button></div>",
            fav = fav_btn, id = item.id
        )
    } else { String::new() };
    format!(
        r#"<li class="food-item-card" id="food-item-{id}" data-fav="{fav}" data-recent="{rec}" data-protein="{p}" data-cal="{cal}">
  {img}
  <div class="food-info">
    <strong>{name} {brand}</strong>
    <span class="food-macros">{cal_s} cal · P {p_s} · C {c_s} · F {f_s}</span>
  </div>
  {pkg}
  {admin}
</li>"#,
        id = item.id,
        fav = if item.is_favourite != 0 { 1 } else { 0 },
        rec = if is_recent { 1 } else { 0 },
        p = item.protein, cal = item.calories,
        img = img_html, name = html_escape(&item.name), brand = brand_html,
        cal_s = fmt_nutrient(item.calories), p_s = fmt_nutrient(item.protein),
        c_s = fmt_nutrient(item.carbs), f_s = fmt_nutrient(item.fat),
        pkg = pkg_badge, admin = admin_btns
    )
}

pub fn library_list_html(items: &[crate::models::FoodItem], recent_ids: &std::collections::HashSet<i64>, is_admin: bool) -> String {
    use std::collections::BTreeMap;
    let mut groups: BTreeMap<String, Vec<&crate::models::FoodItem>> = BTreeMap::new();
    for item in items {
        let key = if item.category.is_empty() { "zzz_Uncategorised".to_string() } else { item.category.clone() };
        groups.entry(key).or_default().push(item);
    }
    groups.iter().map(|(key, group)| {
        let label = key.strip_prefix("zzz_").unwrap_or(key);
        let cards: String = group.iter()
            .map(|i| food_item_card_html(i, recent_ids.contains(&i.id), is_admin))
            .collect::<Vec<_>>().join("\n");
        format!(
            "<div class=\"lib-group\"><div class=\"noc-kicker lib-group-head\">{}</div>\n<ul class=\"food-library-list\">\n{}\n</ul></div>",
            html_escape(label), cards
        )
    }).collect::<Vec<_>>().join("\n")
}
```

Every `library_list_html` caller builds `recent_ids` first:

```rust
    let recent_ids: std::collections::HashSet<i64> =
        crate::db::get_recent_foods(&state.pool, 20).await.into_iter().map(|r| r.food_item_id).collect();
```

The existing `food_item_card` handler (route `GET /api/nutrition/food-items/{id}/card`) and any other direct `food_item_card_html` caller now needs the middle argument — pass `false` there (a single refreshed card doesn't need recency shading):

```rust
        Some(item) => Html(food_item_card_html(&item, false, true)).into_response(),
```

Favourite route + handler:

```rust
async fn toggle_favourite_handler(
    AuthSession(_): AuthSession,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    crate::db::toggle_food_favourite(&state.pool, id).await;
    let items = crate::db::get_food_items(&state.pool).await;
    let recent_ids: std::collections::HashSet<i64> =
        crate::db::get_recent_foods(&state.pool, 20).await.into_iter().map(|r| r.food_item_id).collect();
    Html(library_list_html(&items, &recent_ids, true))
}
```

```rust
        .route("/api/nutrition/food-items/{id}/favourite", post(toggle_favourite_handler))
```

`edit_food_form_html` (the detail sheet) gains, after the custom-portions label — plus a 14-day history strip appended by its handler:

```html
  <label class="package-size-label">Category<input type="text" name="category" value="{category}" placeholder="e.g. Dairy &amp; eggs" list="category-list"></label>
  <label class="package-size-label">Default portion (g)<input type="number" name="default_portion_g" step="0.1" min="0" value="{default_portion}" placeholder="usual amount"></label>
  <label class="fav-label"><input type="checkbox" name="is_favourite" value="1"{fav_checked}> Favourite</label>
```

(with `category = html_escape(&item.category)`, `default_portion = item.default_portion_g.map(|g| fmt_nutrient(g)).unwrap_or_default()`, `fav_checked = if item.is_favourite != 0 { " checked" } else { "" }`). Change `edit_food_form_html` to `edit_food_form_html(item: &FoodItem, history_html: &str)` and interpolate `{history_html}` into the format string just above the `form-actions` div. The `edit_food_form` handler computes the last-14-days strip (dates from `chrono::Utc::now() - Duration::days(13)` to today, values filled per-day from `get_item_log_history`, missing days = 0.0) and passes it in:

```rust
    let hist = history_bars_html(&days);
    Html(edit_food_form_html(&item, &hist)).into_response()
```

```rust
fn history_bars_html(days: &[(String, f64)]) -> String {
    let max = days.iter().map(|(_, g)| *g).fold(0.0f64, f64::max).max(1.0);
    let bars: String = days.iter().map(|(_, g)| {
        let pct = (g / max * 100.0).round();
        format!("<div class=\"hist-bar\" style=\"height:{}%\"></div>", pct.max(if *g > 0.0 { 6.0 } else { 0.0 }))
    }).collect();
    format!("<div class=\"noc-kicker\">Last 14 days</div><div class=\"hist-strip\">{}</div>", bars)
}
```

Filter chips row in `feed.html` above `#food-library`:

```html
    <div class="lib-chips">
      <button class="noc-tag noc-tag-accent" data-filter="all" onclick="libFilter(this)">All</button>
      <button class="noc-tag noc-tag-outline" data-filter="fav" onclick="libFilter(this)">Favourites</button>
      <button class="noc-tag noc-tag-outline" data-filter="recent" onclick="libFilter(this)">Recent</button>
      <button class="noc-tag noc-tag-outline" data-filter="protein" onclick="libFilter(this)">High protein</button>
      <button class="noc-tag noc-tag-outline" data-filter="nomacros" onclick="libFilter(this)">No macros yet</button>
    </div>
```

```js
function libFilter(btn) {
  document.querySelectorAll('.lib-chips [data-filter]').forEach(b => {
    b.classList.toggle('noc-tag-accent', b === btn);
    b.classList.toggle('noc-tag-outline', b !== btn);
  });
  const mode = btn.dataset.filter;
  document.querySelectorAll('#food-library .food-item-card').forEach(card => {
    const show =
      mode === 'all' ? true :
      mode === 'fav' ? card.dataset.fav === '1' :
      mode === 'recent' ? card.dataset.recent === '1' :
      mode === 'protein' ? parseFloat(card.dataset.protein) >= 15 :
      parseFloat(card.dataset.cal) === 0;
    card.style.display = show ? '' : 'none';
  });
  document.querySelectorAll('#food-library .lib-group').forEach(g => {
    const any = [...g.querySelectorAll('.food-item-card')].some(c => c.style.display !== 'none');
    g.style.display = any ? '' : 'none';
  });
}
```

Add-sheet Favourites tab: add `<button class="noc-tag noc-tag-outline" data-tab="favs" onclick="sheetTab('favs')">Favourites</button>` to the sheet tabs, a matching pane

```html
    <div id="sheet-favs" class="sheet-pane" hidden
         hx-get="/fitness/htmx/favourites" hx-trigger="sheet-open from:body" hx-target="find .chips" hx-swap="innerHTML">
      <div class="noc-kicker">Favourites</div>
      <div class="chips"></div>
    </div>
```

extend the `sheetTab` array to `['scan','recent','favs','search']`, and the handler:

```rust
async fn favourite_chips(
    AuthSession(_): AuthSession,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let items = crate::db::get_food_items(&state.pool).await;
    let chips: String = items.iter().filter(|i| i.is_favourite != 0).map(|i| format!(
        r##"<button type="button" class="noc-btn noc-btn-secondary recent-chip"
             hx-get="/fitness/htmx/match-card/{id}" hx-target="#sheet-result" hx-swap="innerHTML">{name}</button>"##,
        id = i.id, name = html_escape(&i.name)
    )).collect();
    Html(if chips.is_empty() { "<p class=\"sheet-hint\">No favourites yet — star items in the library.</p>".to_string() } else { chips })
}
```

```rust
        .route("/fitness/htmx/favourites", get(favourite_chips))
```

Also: in `match_card_html`, when `item.default_portion_g` is `Some(g)`, put it as the **first** portion button labelled `"Usual {g} g"` (before package fractions) — the mockup-2b "usual portion" behavior.

- [ ] **Step 5: CSS**

```css
/* library grouping + detail (Task 8) */
body.fitness-dark .lib-chips { display: flex; gap: 7px; flex-wrap: wrap; margin-bottom: 14px; }
body.fitness-dark .lib-group { margin-bottom: 16px; }
body.fitness-dark .lib-group-head { margin-bottom: 8px; }
body.fitness-dark .food-thumb-empty { background: var(--noc-accent-900); }
body.fitness-dark .food-pkg-badge { flex: none; font-size: 10px; padding: 4px 8px; }
body.fitness-dark .fav-btn {
  background: none; border: none; cursor: pointer; color: var(--noc-faint);
  font-size: 16px; min-width: 40px; min-height: 44px;
}
body.fitness-dark .fav-btn.is-fav { color: var(--noc-accent-400); }
body.fitness-dark .fav-label { display: flex; align-items: center; gap: 8px; font-size: 13px; color: var(--noc-muted); }
body.fitness-dark .hist-strip { display: flex; align-items: flex-end; gap: 5px; height: 52px; margin-top: 8px; }
body.fitness-dark .hist-bar { flex: 1; background: var(--noc-n800); border-radius: 2px; }
body.fitness-dark .hist-strip .hist-bar:last-child { background: var(--noc-accent); }
```

- [ ] **Step 6: Verify + commit**

Run: `cargo fmt --check && cargo clippy && SQLX_OFFLINE=true cargo test` — Expected: green. Browser vs mockups 1c/1d: grouped library with working chips, star toggling, detail form carrying category/favourite/default portion and the 14-day strip; add sheet has a working Favourites tab; a food with a default portion shows "Usual N g" first on its match card.

```bash
git add -A && git commit -m "feat(fitness): grouped library, favourites, food detail with history (migration 010)"
```

---

### Task 9: Week view, weight log, saved meals (migration 011)

**Files:**
- Create: `migrations/011_weights_recipes.sql`
- Create: `templates/fitness/week.html`
- Modify: `src/db.rs` (weights + recipes functions, protein range query, tests)
- Modify: `src/models.rs` (`RecipeWithTotals`)
- Modify: `src/routes/nutrition.rs` (week page handler, weights + recipes handlers, Meals tab, save-as-meal)
- Modify: `templates/fitness/feed.html` (week link in header, Meals tab pane)
- Modify: `static/palette.js` (week command)
- Modify: `static/style.css`

**Interfaces:**
- Consumes: `get_calories_by_date_range` (Task 5), `week_for` (Task 5), `match_card` sheet plumbing (Task 7), slot machinery (Task 4).
- Produces: `db::upsert_weight(pool, date: &str, kg: f64)`, `db::get_weights_since(pool, start: &str) -> Vec<(String, f64)>`, `db::get_latest_weight(pool) -> Option<(String, f64)>`; `db::get_protein_by_date_range(pool, start, end) -> Vec<(String, f64)>`; `db::get_logged_dates_desc(pool, limit: i64) -> Vec<String>`; `db::get_most_logged_between(pool, start, end, limit) -> Vec<(String, i64)>` (name, count); `pub struct RecipeWithTotals { pub id: i64, pub name: String, pub item_count: i64, pub total_cal: f64 }`; `db::create_recipe_from_slot(pool, name: &str, date: &str, slot: &str) -> Option<i64>` (None when the slot is empty), `db::get_recipes_with_totals(pool) -> Vec<RecipeWithTotals>`, `db::log_recipe(pool, id: i64, date: &str, slot: &str) -> u64`, `db::delete_recipe(pool, id: i64)`; pure fn `compute_streak(logged_desc: &[String], today: &str) -> i64` in `nutrition.rs`; routes `GET /fitness/week`, `POST /api/nutrition/weights`, `POST /api/nutrition/recipes`, `POST /api/nutrition/recipes/{id}/log`, `DELETE /api/nutrition/recipes/{id}`, `GET /fitness/htmx/meals`.

- [ ] **Step 1: Migration + apply**

`migrations/011_weights_recipes.sql`:

```sql
CREATE TABLE IF NOT EXISTS weights (
    date TEXT PRIMARY KEY,
    kg REAL NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE TABLE IF NOT EXISTS recipes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE TABLE IF NOT EXISTS recipe_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    recipe_id INTEGER NOT NULL,
    food_item_id INTEGER NOT NULL,
    grams REAL NOT NULL
);
```

Register after 010; apply with the python3 one-liner.

- [ ] **Step 2: Failing db tests**

```rust
    #[tokio::test]
    async fn test_weight_upsert_and_range() {
        let pool = test_pool().await;
        upsert_weight(&pool, "2026-07-30", 82.7).await;
        upsert_weight(&pool, "2026-07-31", 82.4).await;
        upsert_weight(&pool, "2026-07-31", 82.5).await; // same-day overwrite
        let all = get_weights_since(&pool, "2026-07-01").await;
        assert_eq!(all.len(), 2);
        assert_eq!(all[1], ("2026-07-31".to_string(), 82.5));
        assert_eq!(get_latest_weight(&pool).await, Some(("2026-07-31".to_string(), 82.5)));
    }

    #[tokio::test]
    async fn test_recipe_create_and_log() {
        let pool = test_pool().await;
        let a = insert_food_item(&pool, "Oats", "", None, 379.0, 13.0, 60.0, 6.5, 0.0, 0.0, 0.0, 0.0, None, "", "").await;
        let b = insert_food_item(&pool, "Skyr", "", None, 63.0, 11.0, 4.0, 0.2, 0.0, 0.0, 0.0, 0.0, None, "", "").await;
        insert_meal_entry(&pool, a.id, "2026-07-31", 80.0, "breakfast").await.unwrap();
        insert_meal_entry(&pool, b.id, "2026-07-31", 250.0, "breakfast").await.unwrap();
        assert!(create_recipe_from_slot(&pool, "Overnight oats", "2026-07-31", "dinner").await.is_none()); // empty slot
        let rid = create_recipe_from_slot(&pool, "Overnight oats", "2026-07-31", "breakfast").await.unwrap();
        let recipes = get_recipes_with_totals(&pool).await;
        assert_eq!(recipes.len(), 1);
        assert_eq!(recipes[0].item_count, 2);
        assert!((recipes[0].total_cal - (379.0 * 0.8 + 63.0 * 2.5)).abs() < 0.1);
        let inserted = log_recipe(&pool, rid, "2026-08-01", "snack").await;
        assert_eq!(inserted, 2);
        let entries = get_meal_entries_for_date(&pool, "2026-08-01").await;
        assert!(entries.iter().all(|e| e.slot == "snack"));
        delete_recipe(&pool, rid).await;
        assert!(get_recipes_with_totals(&pool).await.is_empty());
    }

    #[tokio::test]
    async fn test_protein_range_and_logged_dates() {
        let pool = test_pool().await;
        let a = insert_food_item(&pool, "Chicken", "", None, 165.0, 31.0, 0.0, 3.6, 0.0, 0.0, 0.0, 0.0, None, "", "").await;
        insert_meal_entry(&pool, a.id, "2026-07-30", 200.0, "lunch").await.unwrap();
        insert_meal_entry(&pool, a.id, "2026-07-31", 100.0, "lunch").await.unwrap();
        let prot = get_protein_by_date_range(&pool, "2026-07-30", "2026-07-31").await;
        assert_eq!(prot.len(), 2);
        assert!((prot[0].1 - 62.0).abs() < 0.01);
        assert_eq!(get_logged_dates_desc(&pool, 10).await, vec!["2026-07-31", "2026-07-30"]);
        let most = get_most_logged_between(&pool, "2026-07-27", "2026-08-02", 5).await;
        assert_eq!(most[0], ("Chicken".to_string(), 2));
    }
```

Run: expected FAIL.

- [ ] **Step 3: Implement db + model**

`src/models.rs`:

```rust
#[derive(Debug, Clone)]
pub struct RecipeWithTotals {
    pub id: i64,
    pub name: String,
    pub item_count: i64,
    pub total_cal: f64,
}
```

`src/db.rs`:

```rust
pub async fn upsert_weight(pool: &DbPool, date: &str, kg: f64) {
    sqlx::query!(
        "INSERT INTO weights (date, kg) VALUES (?, ?) ON CONFLICT(date) DO UPDATE SET kg = excluded.kg",
        date, kg
    )
    .execute(pool)
    .await
    .ok();
}

pub async fn get_weights_since(pool: &DbPool, start: &str) -> Vec<(String, f64)> {
    sqlx::query!(r#"SELECT date as "date!", kg as "kg!: f64" FROM weights WHERE date >= ? ORDER BY date ASC"#, start)
        .fetch_all(pool)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|r| (r.date, r.kg))
        .collect()
}

pub async fn get_latest_weight(pool: &DbPool) -> Option<(String, f64)> {
    sqlx::query!(r#"SELECT date as "date!", kg as "kg!: f64" FROM weights ORDER BY date DESC LIMIT 1"#)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
        .map(|r| (r.date, r.kg))
}

pub async fn get_protein_by_date_range(pool: &DbPool, start: &str, end: &str) -> Vec<(String, f64)> {
    sqlx::query!(
        r#"SELECT me.date as "date!", SUM(me.grams / 100.0 * fi.protein) as "protein!: f64"
        FROM meal_entries me JOIN food_items fi ON fi.id = me.food_item_id
        WHERE me.date >= ? AND me.date <= ?
        GROUP BY me.date ORDER BY me.date ASC"#,
        start, end
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|r| (r.date, r.protein))
    .collect()
}

pub async fn get_logged_dates_desc(pool: &DbPool, limit: i64) -> Vec<String> {
    sqlx::query!(r#"SELECT DISTINCT date as "date!" FROM meal_entries ORDER BY date DESC LIMIT ?"#, limit)
        .fetch_all(pool)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|r| r.date)
        .collect()
}

pub async fn get_most_logged_between(pool: &DbPool, start: &str, end: &str, limit: i64) -> Vec<(String, i64)> {
    sqlx::query!(
        r#"SELECT fi.name as "name!", COUNT(*) as "n!: i64"
        FROM meal_entries me JOIN food_items fi ON fi.id = me.food_item_id
        WHERE me.date >= ? AND me.date <= ?
        GROUP BY me.food_item_id ORDER BY 2 DESC, fi.name ASC LIMIT ?"#,
        start, end, limit
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|r| (r.name, r.n))
    .collect()
}

pub async fn create_recipe_from_slot(pool: &DbPool, name: &str, date: &str, slot: &str) -> Option<i64> {
    let mut tx = pool.begin().await.ok()?;
    let rows = sqlx::query!(
        "SELECT food_item_id, grams FROM meal_entries WHERE date = ? AND slot = ?",
        date, slot
    )
    .fetch_all(&mut *tx)
    .await
    .ok()?;
    if rows.is_empty() { return None; }
    let rid = sqlx::query!("INSERT INTO recipes (name) VALUES (?) RETURNING id", name)
        .fetch_one(&mut *tx)
        .await
        .ok()?
        .id;
    for r in rows {
        sqlx::query!(
            "INSERT INTO recipe_items (recipe_id, food_item_id, grams) VALUES (?, ?, ?)",
            rid, r.food_item_id, r.grams
        )
        .execute(&mut *tx)
        .await
        .ok()?;
    }
    tx.commit().await.ok()?;
    Some(rid)
}

pub async fn get_recipes_with_totals(pool: &DbPool) -> Vec<crate::models::RecipeWithTotals> {
    sqlx::query!(
        r#"SELECT r.id as "id!", r.name as "name!",
                  COUNT(ri.id) as "item_count!: i64",
                  COALESCE(SUM(ri.grams / 100.0 * fi.calories), 0) as "total_cal!: f64"
        FROM recipes r
        LEFT JOIN recipe_items ri ON ri.recipe_id = r.id
        LEFT JOIN food_items fi ON fi.id = ri.food_item_id
        GROUP BY r.id ORDER BY r.name ASC"#
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|r| crate::models::RecipeWithTotals { id: r.id, name: r.name, item_count: r.item_count, total_cal: r.total_cal })
    .collect()
}

pub async fn log_recipe(pool: &DbPool, id: i64, date: &str, slot: &str) -> u64 {
    sqlx::query!(
        "INSERT INTO meal_entries (food_item_id, date, grams, slot)
         SELECT food_item_id, ?, grams, ? FROM recipe_items WHERE recipe_id = ?",
        date, slot, id
    )
    .execute(pool)
    .await
    .map(|r| r.rows_affected())
    .unwrap_or(0)
}

pub async fn delete_recipe(pool: &DbPool, id: i64) {
    sqlx::query!("DELETE FROM recipe_items WHERE recipe_id = ?", id).execute(pool).await.ok();
    sqlx::query!("DELETE FROM recipes WHERE id = ?", id).execute(pool).await.ok();
}
```

`DATABASE_URL=sqlite:portfolio.db cargo sqlx prepare && SQLX_OFFLINE=true cargo test` — Expected: PASS.

- [ ] **Step 4: Streak helper (TDD)** — add to `nutrition.rs` tests:

```rust
    #[test]
    fn test_compute_streak() {
        let d = |s: &str| s.to_string();
        assert_eq!(compute_streak(&[], "2026-08-01"), 0);
        assert_eq!(compute_streak(&[d("2026-08-01"), d("2026-07-31"), d("2026-07-30")], "2026-08-01"), 3);
        // today not yet logged still counts yesterday's run
        assert_eq!(compute_streak(&[d("2026-07-31"), d("2026-07-30")], "2026-08-01"), 2);
        // gap breaks it
        assert_eq!(compute_streak(&[d("2026-08-01"), d("2026-07-29")], "2026-08-01"), 1);
    }
```

Run → FAIL, then implement:

```rust
fn compute_streak(logged_desc: &[String], today: &str) -> i64 {
    use chrono::{NaiveDate, Duration};
    let Ok(today) = NaiveDate::parse_from_str(today, "%Y-%m-%d") else { return 0; };
    let mut expect = today;
    let mut streak = 0i64;
    for (i, d) in logged_desc.iter().enumerate() {
        let Ok(d) = NaiveDate::parse_from_str(d, "%Y-%m-%d") else { break; };
        if i == 0 && d == today - Duration::days(1) {
            expect = d; // today not logged yet — start from yesterday
        }
        if d == expect {
            streak += 1;
            expect -= Duration::days(1);
        } else if d < expect {
            break;
        }
    }
    streak
}
```

Run → PASS.

- [ ] **Step 5: Week page**

`templates/fitness/week.html`:

```html
{% extends "base.html" %}
{% block title %}Fitness · Week{% endblock %}
{% block body_class %}fitness-dark{% endblock %}

{% block head %}
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600&display=swap">
{% endblock %}

{% block content %}
<section class="fitness-page week-page">
  <div class="week-head">
    <div>
      <div class="noc-kicker">This week</div>
      <h1 class="week-title">{{ range_label }}</h1>
    </div>
    <a class="noc-btn noc-btn-secondary" href="/fitness">Today</a>
  </div>

  <div class="noc-card week-chart-card">
    <div class="week-chart-head">
      <span class="noc-kicker">Calories vs {{ target_cal }} target</span>
      <span class="week-avg">avg {{ avg_cal }}</span>
    </div>
    {{ bars_html|safe }}
  </div>

  <div class="week-stats">
    <div class="noc-card stat-card">
      <div class="noc-kicker">Protein avg</div>
      <div class="stat-big">{{ protein_avg }} g</div>
      <div class="stat-sub">target {{ target_protein }} · {{ protein_hits }} days hit</div>
    </div>
    <div class="noc-card stat-card">
      <div class="noc-kicker">Days logged</div>
      <div class="stat-big">{{ days_logged }} / 7</div>
      <div class="stat-sub">streak {{ streak }} days</div>
    </div>
  </div>

  <div class="noc-card weight-card" id="weight-card">
    {{ weight_card_html|safe }}
  </div>

  <div class="most-logged">
    <div class="noc-kicker">Most logged this week</div>
    {{ most_logged_html|safe }}
  </div>
</section>
{% endblock %}
```

Handler (all values pre-computed; bars are buttons linking into days):

```rust
#[derive(Template)]
#[template(path = "fitness/week.html")]
struct WeekTemplate {
    is_admin: bool,
    range_label: String,
    target_cal: String,
    avg_cal: String,
    bars_html: String,
    protein_avg: String,
    target_protein: String,
    protein_hits: i64,
    days_logged: i64,
    streak: i64,
    weight_card_html: String,
    most_logged_html: String,
}

fn weight_card_html(latest: Option<(String, f64)>, series: &[(String, f64)]) -> String {
    let (val, sub) = match &latest {
        Some((date, kg)) => (format!("{:.1}", kg), format!("kg · {}", html_escape(date))),
        None => ("—".to_string(), "no weight logged yet".to_string()),
    };
    let delta = if series.len() >= 2 {
        let d = series.last().unwrap().1 - series.first().unwrap().1;
        format!(r#"<span class="weight-delta">{}{:.1} kg / 30 d</span>"#, if d <= 0.0 { "−" } else { "+" }, d.abs())
    } else { String::new() };
    let line = if series.len() >= 2 {
        let min = series.iter().map(|(_, k)| *k).fold(f64::INFINITY, f64::min);
        let max = series.iter().map(|(_, k)| *k).fold(f64::NEG_INFINITY, f64::max);
        let span = (max - min).max(0.5);
        let pts: Vec<String> = series.iter().enumerate().map(|(i, (_, k))| {
            let x = i as f64 / (series.len() - 1) as f64 * 320.0;
            let y = 8.0 + (max - k) / span * 44.0;
            format!("{:.0},{:.1}", x, y)
        }).collect();
        format!(
            r#"<svg viewBox="0 0 320 60" width="100%" height="60" preserveAspectRatio="none"><polyline points="{}" fill="none" stroke="#9184d9" stroke-width="2" stroke-linecap="round" style="filter:drop-shadow(0 0 5px rgba(145,132,217,.5))"></polyline></svg>"#,
            pts.join(" ")
        )
    } else { String::new() };
    format!(
        r##"<div class="week-chart-head"><span class="noc-kicker">Weight</span>{delta}</div>
<div class="weight-now"><span class="stat-big">{val}</span><span class="stat-sub">{sub}</span></div>
{line}
<form class="weight-form" hx-post="/api/nutrition/weights" hx-target="#weight-card" hx-swap="innerHTML">
  <input class="noc-input" type="number" name="kg" step="0.1" min="20" max="400" placeholder="kg" required>
  <button type="submit" class="noc-btn noc-btn-secondary">Log today's weight</button>
</form>"##,
        delta = delta, val = val, sub = sub, line = line
    )
}

async fn week_page(
    AuthSession(_): AuthSession,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    use chrono::{Duration, NaiveDate};
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let targets = crate::db::get_targets(&state.pool).await;
    let week = week_for(&state.pool, &today).await;
    let start = &week[0].0;
    let end = &week[6].0;

    let logged: Vec<&(String, f64)> = week.iter().filter(|(d, c)| *c > 0.0 && d.as_str() <= today.as_str()).collect();
    let avg_cal = if logged.is_empty() { 0.0 } else { logged.iter().map(|(_, c)| c).sum::<f64>() / logged.len() as f64 };

    let bars: String = week.iter().map(|(d, c)| {
        let pct = if targets.calories > 0.0 { (c / targets.calories * 100.0).clamp(0.0, 112.0) } else { 0.0 };
        let cls = if *d == today { "wk-bar today" } else if d.as_str() > today.as_str() { "wk-bar future" } else { "wk-bar" };
        format!(
            r##"<a class="{cls}" href="/fitness?date={d}" style="--h:{pct:.0}%" aria-label="{d}"></a>"##,
            cls = cls, d = d, pct = pct
        )
    }).collect();
    // the target line sits at 100/112 of the clamped bar scale — fixed in CSS (bottom: 89.3%)
    let bars_html = format!(
        r##"<div class="wk-chart"><div class="wk-target-line"></div>{bars}</div>
<div class="wk-letters"><span>S</span><span>M</span><span>T</span><span>W</span><span>T</span><span>F</span><span>S</span></div>"##,
        bars = bars
    );

    let prot = crate::db::get_protein_by_date_range(&state.pool, start, end).await;
    let protein_avg = if prot.is_empty() { 0.0 } else { prot.iter().map(|(_, p)| p).sum::<f64>() / prot.len() as f64 };
    let protein_hits = prot.iter().filter(|(_, p)| *p >= targets.protein).count() as i64;
    let days_logged = logged.len() as i64;
    let streak = compute_streak(&crate::db::get_logged_dates_desc(&state.pool, 400).await, &today);

    let month_ago = NaiveDate::parse_from_str(&today, "%Y-%m-%d")
        .map(|d| (d - Duration::days(30)).format("%Y-%m-%d").to_string())
        .unwrap_or_default();
    let weights = crate::db::get_weights_since(&state.pool, &month_ago).await;
    let latest = crate::db::get_latest_weight(&state.pool).await;

    let most = crate::db::get_most_logged_between(&state.pool, start, end, 5).await;
    let most_logged_html: String = most.iter().map(|(name, n)| format!(
        r#"<div class="most-row"><span class="most-name">{}</span><span class="most-n">{}×</span></div>"#,
        html_escape(name), n
    )).collect();

    let range_label = {
        let s = NaiveDate::parse_from_str(start, "%Y-%m-%d").unwrap();
        let e = NaiveDate::parse_from_str(end, "%Y-%m-%d").unwrap();
        format!("{} – {}", s.format("%-d %b"), e.format("%-d %b"))
    };

    Html(WeekTemplate {
        is_admin: true,
        range_label,
        target_cal: format!("{:.0}", targets.calories),
        avg_cal: format!("{:.0}", avg_cal),
        bars_html,
        protein_avg: format!("{:.0}", protein_avg),
        target_protein: format!("{:.0} g", targets.protein),
        protein_hits,
        days_logged,
        streak,
        weight_card_html: weight_card_html(latest, &weights),
        most_logged_html,
    }.render().unwrap())
}

async fn log_weight_handler(
    AuthSession(_): AuthSession,
    State(state): State<Arc<AppState>>,
    axum::Form(form): axum::Form<HashMap<String, String>>,
) -> impl IntoResponse {
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    if let Some(kg) = form.get("kg").and_then(|v| v.parse::<f64>().ok()) {
        if (20.0..=400.0).contains(&kg) {
            crate::db::upsert_weight(&state.pool, &today, kg).await;
        }
    }
    use chrono::{Duration, NaiveDate};
    let month_ago = NaiveDate::parse_from_str(&today, "%Y-%m-%d")
        .map(|d| (d - Duration::days(30)).format("%Y-%m-%d").to_string())
        .unwrap_or_default();
    let weights = crate::db::get_weights_since(&state.pool, &month_ago).await;
    let latest = crate::db::get_latest_weight(&state.pool).await;
    Html(weight_card_html(latest, &weights))
}
```

- [ ] **Step 6: Saved meals (create / log / list)**

Handlers:

```rust
async fn create_recipe_handler(
    AuthSession(_): AuthSession,
    State(state): State<Arc<AppState>>,
    axum::Form(form): axum::Form<HashMap<String, String>>,
) -> impl IntoResponse {
    let name = form.get("name").map(|s| s.trim()).unwrap_or("");
    let date = form.get("date").cloned().unwrap_or_default();
    let slot = form.get("slot").cloned().unwrap_or_default();
    if !name.is_empty() {
        crate::db::create_recipe_from_slot(&state.pool, name, &date, &slot).await;
    }
    let targets = crate::db::get_targets(&state.pool).await;
    let entries = crate::db::get_meal_entries_for_date(&state.pool, &date).await;
    let food_items = crate::db::get_food_items(&state.pool).await;
    Html(day_section_html(&entries, &date, &food_items, &targets, true))
}

async fn log_recipe_handler(
    AuthSession(_): AuthSession,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    axum::Form(form): axum::Form<HashMap<String, String>>,
) -> impl IntoResponse {
    let date = form.get("date").cloned().unwrap_or_else(|| chrono::Utc::now().format("%Y-%m-%d").to_string());
    let slot = form.get("slot").cloned().unwrap_or_else(|| "other".to_string());
    let slot = if SLOTS.iter().any(|(k, _)| *k == slot) { slot } else { "other".to_string() };
    crate::db::log_recipe(&state.pool, id, &date, &slot).await;
    let targets = crate::db::get_targets(&state.pool).await;
    let entries = crate::db::get_meal_entries_for_date(&state.pool, &date).await;
    let food_items = crate::db::get_food_items(&state.pool).await;
    Html(day_section_html(&entries, &date, &food_items, &targets, true))
}

async fn meals_pane(
    AuthSession(_): AuthSession,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let recipes = crate::db::get_recipes_with_totals(&state.pool).await;
    let rows: String = recipes.iter().map(|r| format!(
        r##"<div class="meal-row">
  <form hx-post="/api/nutrition/recipes/{id}/log" hx-target="#day-section" hx-swap="innerHTML" hx-on::after-request="closeAddSheet()">
    <input type="hidden" name="date" value=""><input type="hidden" name="slot" value="other">
    <button type="submit" class="noc-btn noc-btn-secondary meal-log-btn"><span>{name}</span><span class="meal-cal">{cal} cal</span></button>
  </form>
  <button class="food-delete-btn" hx-delete="/api/nutrition/recipes/{id}" hx-target="#sheet-meals .chips" hx-swap="innerHTML" hx-confirm="Delete this saved meal?">×</button>
</div>"##,
        id = r.id, name = html_escape(&r.name), cal = format!("{:.0}", r.total_cal)
    )).collect();
    Html(if rows.is_empty() { "<p class=\"sheet-hint\">No saved meals yet — save a day's slot from the Today view.</p>".to_string() } else { rows })
}

async fn delete_recipe_handler(
    AuthSession(_): AuthSession,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    crate::db::delete_recipe(&state.pool, id).await;
    meals_pane(AuthSession(String::new()), State(state)).await
}
```

(the `delete_recipe_handler` re-invocation trick is fine — `AuthSession` already ran; if the borrow checker objects, inline the same body instead.)

Routes:

```rust
        .route("/fitness/week", get(week_page))
        .route("/api/nutrition/weights", post(log_weight_handler))
        .route("/api/nutrition/recipes", post(create_recipe_handler))
        .route("/api/nutrition/recipes/{id}/log", post(log_recipe_handler))
        .route("/api/nutrition/recipes/{id}", delete(delete_recipe_handler))
        .route("/fitness/htmx/meals", get(meals_pane))
```

Sheet gets a Meals tab (same pattern as Favourites: tab button `data-tab="meals"`, pane `#sheet-meals` with `hx-get="/fitness/htmx/meals"`, add `'meals'` to the `sheetTab` array). Meal-row date/slot hidden inputs are filled by the `initMatchCard`-style hook — extend it:

```js
function initSheetForms() {
  document.querySelectorAll('#add-sheet form').forEach(f => {
    const d = f.querySelector('[name="date"]');
    if (d && !d.value) d.value = window.currentDate;
    const s = f.querySelector('[name="slot"]');
    if (s && s.value === 'other') s.value = defaultSlot();
  });
}
document.body.addEventListener('htmx:afterSwap', initSheetForms);
```

"Save as meal" affordance: in `day_section_html`'s slot loop, for non-empty slots append after the `</ul>`:

```rust
format!(
    r##"<details class="save-meal"><summary>Save as meal</summary>
<form hx-post="/api/nutrition/recipes" hx-target="#day-section" hx-swap="innerHTML">
  <input type="hidden" name="date" value="{date}"><input type="hidden" name="slot" value="{key}">
  <input class="noc-input" type="text" name="name" placeholder="Meal name" required>
  <button type="submit" class="noc-btn noc-btn-secondary">Save</button>
</form></details>"##,
    date = html_escape(date), key = key
)
```

- [ ] **Step 7: Header link + palette + CSS**

`feed.html` header row gains `<a class="noc-btn noc-btn-ghost" href="/fitness/week">Week →</a>` next to the arrows. `static/palette.js` COMMANDS array gains:

```js
  {
    label: 'Go to Fitness Week',
    keywords: ['week', 'trends', 'weight', 'streak', 'fitness'],
    action() { location.href = '/fitness/week'; },
  },
```

CSS:

```css
/* week page + saved meals (Task 9) */
body.fitness-dark .week-head { display: flex; align-items: flex-end; justify-content: space-between; margin: 14px 0 18px; }
body.fitness-dark .week-title { font-size: 22px; font-weight: 500; letter-spacing: -.015em; margin: 2px 0 0; }
body.fitness-dark .week-chart-card { margin-bottom: 14px; }
body.fitness-dark .week-chart-head { display: flex; justify-content: space-between; align-items: baseline; margin-bottom: 12px; }
body.fitness-dark .week-avg { font-size: 12px; color: var(--noc-muted); }
body.fitness-dark .wk-chart { position: relative; height: 138px; display: flex; align-items: flex-end; gap: 9px; }
body.fitness-dark .wk-target-line { position: absolute; left: 0; right: 0; bottom: 89.3%; border-top: 1px dashed color-mix(in srgb, var(--noc-accent) 45%, transparent); }
body.fitness-dark .wk-bar { flex: 1; height: var(--h); background: var(--noc-n800); border-radius: 4px 4px 0 0; min-height: 2px; }
body.fitness-dark .wk-bar.today { background: var(--noc-accent); box-shadow: 0 0 14px color-mix(in srgb, var(--noc-accent) 35%, transparent); }
body.fitness-dark .wk-bar.future { background: var(--noc-surface); box-shadow: inset 0 0 0 1px var(--noc-n800); }
body.fitness-dark .wk-letters { display: flex; gap: 9px; margin-top: 8px; }
body.fitness-dark .wk-letters span { flex: 1; text-align: center; font-size: 10px; color: var(--noc-faint); }
body.fitness-dark .week-stats { display: grid; grid-template-columns: 1fr 1fr; gap: 10px; margin-bottom: 14px; }
body.fitness-dark .stat-big { font-size: 26px; font-weight: 500; margin-top: 4px; letter-spacing: -.015em; }
body.fitness-dark .stat-sub { font-size: 11px; color: var(--noc-faint); }
body.fitness-dark .weight-card { margin-bottom: 14px; }
body.fitness-dark .weight-now { display: flex; align-items: baseline; gap: 10px; margin: 6px 0 8px; }
body.fitness-dark .weight-delta { font-size: 12px; color: var(--noc-accent-400); }
body.fitness-dark .weight-form { display: flex; gap: 8px; margin-top: 12px; }
body.fitness-dark .weight-form input { width: 110px; }
body.fitness-dark .most-row { display: flex; align-items: center; gap: 10px; font-size: 13px; padding: 5px 0; }
body.fitness-dark .most-row .most-name { flex: 1; }
body.fitness-dark .most-row .most-n { color: var(--noc-muted); }
body.fitness-dark .save-meal { margin-top: 6px; }
body.fitness-dark .save-meal summary { font-size: 11px; color: var(--noc-faint); cursor: pointer; min-height: 32px; display: flex; align-items: center; }
body.fitness-dark .save-meal form { display: flex; gap: 8px; margin-top: 6px; }
body.fitness-dark .meal-row { display: flex; align-items: center; gap: 6px; margin-top: 8px; }
body.fitness-dark .meal-row form { flex: 1; }
body.fitness-dark .meal-log-btn { width: 100%; justify-content: space-between; }
body.fitness-dark .meal-cal { color: var(--noc-muted); font-size: 12px; }
```

- [ ] **Step 8: Verify + commit**

Run: `cargo fmt --check && cargo clippy && SQLX_OFFLINE=true cargo test` — Expected: green. Browser vs mockup 1e: `/fitness/week` shows bars with the dashed target line, stat tiles, weight card with working log, most-logged list; bar tap opens that day; saving a slot as a meal and re-logging it from the Meals tab works end-to-end.

```bash
git add -A && git commit -m "feat(fitness): week view, weight log, saved meals (migration 011)"
```

---

### Task 10: Desktop width + keyboard quick-add

**Files:**
- Modify: `src/routes/nutrition.rs` (`POST /fitness/quick-log`)
- Modify: `templates/fitness/feed.html` (quick-add row)
- Modify: `static/style.css` (desktop breakpoint)

**Interfaces:**
- Consumes: `GET /fitness/htmx/food-search` (Task 7), `default_portion_g` (Task 8), slot inference (Task 4).
- Produces: `POST /fitness/quick-log` (form: `food_item_id`, `date`, `slot`) — logs the item's `default_portion_g` (fallback 100 g) and returns the day section; quick-add row `#quickadd` visible ≥900px.

- [ ] **Step 1: Handler + route**

```rust
async fn quick_log_handler(
    AuthSession(_): AuthSession,
    State(state): State<Arc<AppState>>,
    axum::Form(form): axum::Form<HashMap<String, String>>,
) -> impl IntoResponse {
    let date = form.get("date").cloned().unwrap_or_else(|| chrono::Utc::now().format("%Y-%m-%d").to_string());
    let slot = form.get("slot").cloned().unwrap_or_else(|| "other".to_string());
    let slot = if SLOTS.iter().any(|(k, _)| *k == slot) { slot } else { "other".to_string() };
    if let Some(id) = form.get("food_item_id").and_then(|v| v.parse::<i64>().ok()) {
        if let Some(item) = crate::db::get_food_item(&state.pool, id).await {
            let grams = item.default_portion_g.unwrap_or(100.0);
            let _ = crate::db::insert_meal_entry(&state.pool, id, &date, grams, &slot).await;
        }
    }
    let targets = crate::db::get_targets(&state.pool).await;
    let entries = crate::db::get_meal_entries_for_date(&state.pool, &date).await;
    let food_items = crate::db::get_food_items(&state.pool).await;
    Html(day_section_html(&entries, &date, &food_items, &targets, true))
}
```

```rust
        .route("/fitness/quick-log", post(quick_log_handler))
```

`food_search` result rows gain the id as a data attribute so quick-add can post it — add `data-item-id="{id}"` to the `.search-row` button in `food_search` (Task 7's builder).

- [ ] **Step 2: Quick-add row** (in `feed.html`, directly under the date header, before the day section)

```html
  <div id="quickadd">
    <input class="noc-input" type="search" id="quickadd-input" placeholder="Type a food, Enter logs your usual portion…"
           name="q" hx-get="/fitness/htmx/food-search" hx-trigger="input changed delay:250ms"
           hx-target="#quickadd-results" hx-swap="innerHTML">
    <div id="quickadd-results"></div>
  </div>
```

```js
function quickLogItem(id) {
  htmx.ajax('POST', '/fitness/quick-log', {
    target: '#day-section', swap: 'innerHTML',
    values: { food_item_id: id, date: window.currentDate, slot: defaultSlot() }
  });
  document.getElementById('quickadd-input').value = '';
  document.getElementById('quickadd-results').innerHTML = '';
}

function initQuickAdd() {
  const input = document.getElementById('quickadd-input');
  if (!input || input.dataset.init) return;
  input.dataset.init = '1';
  input.addEventListener('keydown', e => {
    if (e.key !== 'Enter') return;
    e.preventDefault();
    const first = document.querySelector('#quickadd-results .search-row');
    if (first) quickLogItem(first.dataset.itemId);
  });
  document.getElementById('quickadd-results').addEventListener('click', e => {
    const row = e.target.closest('.search-row');
    if (row) { e.preventDefault(); e.stopPropagation(); quickLogItem(row.dataset.itemId); }
  }, true); // capture so the row's hx-get (sheet flow) doesn't also fire here
}
document.addEventListener('DOMContentLoaded', initQuickAdd);
document.body.addEventListener('htmx:afterSwap', initQuickAdd);
```

- [ ] **Step 3: Desktop CSS**

```css
/* desktop (Task 10) */
body.fitness-dark #quickadd { display: none; position: relative; margin: 0 0 16px; }
body.fitness-dark #quickadd-results { position: absolute; top: 100%; left: 0; right: 0; z-index: 30; background: var(--noc-surface-2); border-radius: var(--noc-radius-md); box-shadow: var(--noc-shadow-pop); }
body.fitness-dark #quickadd-results .search-row { margin-top: 0; border-radius: 0; box-shadow: none; background: transparent; }
body.fitness-dark #quickadd-results .search-row:first-child { background: var(--noc-surface); }
@media (min-width: 900px) {
  body.fitness-dark .fitness-page { max-width: 760px; padding-bottom: 40px; }
  body.fitness-dark #quickadd { display: block; }
  body.fitness-dark #fitness-actionbar { position: static; max-width: none; padding: 16px 0 0; background: none; }
  body.fitness-dark #add-sheet { max-width: 560px; border-radius: var(--noc-radius-lg); inset: 40px auto auto 50%; transform: translateX(-50%); max-height: calc(100vh - 80px); box-shadow: var(--noc-shadow-pop); }
}
```

- [ ] **Step 4: Verify + commit**

Run: `cargo fmt --check && cargo clippy && SQLX_OFFLINE=true cargo test` — Expected: green. Browser at ≥900px: quick-add types ahead over the library, Enter logs the first match's usual portion into the clock slot; the sheet floats as a centered panel; mobile viewport unchanged.

```bash
git add -A && git commit -m "feat(fitness): desktop quick-add and wide layout"
```

---

### Task 11: Docs + final verification

**Files:**
- Modify: `CLAUDE.md` (routes, migrations 008–011, auth note, tests, design-docs pointer)
- Modify: `docs/design.md` (fitness section describes the new IA)

**Interfaces:** none — documentation and a full pass.

- [ ] **Step 1: Update CLAUDE.md**

In the route-modules list, replace the `nutrition.rs` line with the full new surface (page `GET /fitness` (+`?date=`), `GET /fitness/week`, htmx fragments `day`, `targets`, `recent`, `favourites`, `meals`, `food-search`, `match-card/{id}`, `barcode-match/{code}`, `entries/{id}/edit`; actions `POST /fitness/copy-day`, `POST /fitness/quick-log`; API `POST/PUT/DELETE /api/nutrition/entries…`, `food-items…` + `/favourite`, `targets`, `weights`, `recipes…`). Note that **all `/fitness` and `/api/nutrition` routes require `AuthSession`** (decision 2026-08-01). Extend the migrations sentence to "eight migrations exist (… 008 meal slots, 009 targets, 010 food metadata, 011 weights/recipes)". Update the nutrition-tests sentence. Add under Key implementation details: "Fitness UI is the Nocturne dark theme — tokens under `body.fitness-dark` in `style.css`; design reference in `docs/design/fitness-redesign/`."

- [ ] **Step 2: Update docs/design.md**

Add/replace the fitness section with a short description of the new IA (Today with targets ring + slots + week strip, add sheet, grouped library, week view) and a pointer to `docs/design/fitness-redesign/README.md` for the decisions.

- [ ] **Step 3: Full verification**

```bash
cargo fmt --check && cargo clippy && SQLX_OFFLINE=true cargo test
```

Expected: zero warnings-as-errors, all tests green — quote the test summary line in the report. Then a full browser walkthrough with the dev session at 402px-wide and desktop: Today (ring, rails, strip, slots, edit entry, copy yesterday), add sheet (scan tab manual code path, recents, favourites, meals, search, portion buttons, log), library (chips, star, detail edit save round-trip), week page (bars, stats, weight log), quick-add on desktop, and hx-boost navigation Fitness → Hub → Fitness (scripts must still work — guarded listeners).

- [ ] **Step 4: Commit**

```bash
git add -A && git commit -m "docs: fitness redesign — CLAUDE.md and design docs updated"
```
