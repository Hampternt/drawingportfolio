# Portfolio Site — Design Guide

A short reference for how this project is structured and where new code belongs.
Intended to keep scope clean as the project grows.

---

## Layers and what lives in each

```
nginx  →  Rust/Axum  →  SQLite / S3
             ↓
         templates/
             ↓
         static/
```

| Layer | Files | Responsibility |
|---|---|---|
| **Routing** | `src/routes/*.rs` | Accept HTTP requests, call db, render template |
| **Data** | `src/db.rs` | All SQL queries — nothing else |
| **Models** | `src/models.rs` | Shared data structs used across routes |
| **Storage** | `src/storage.rs` | S3 upload/delete only |
| **Templates** | `templates/` | HTML structure and layout — no business logic |
| **Styles** | `static/style.css` | Visual appearance only |
| **Behaviour** | `static/*.js` | Client-side interactivity |
| **Reverse proxy** | `deploy/nginx.conf` | TLS, rate limiting, caching headers, gzip |

**Rule:** code belongs in the layer that owns its concern. A route handler should not
build raw SQL strings. A template should not make routing decisions. A JS file should
not know about database IDs beyond what the server gave it.

---

## Page templates

Every user-facing page must extend `base.html`:

```html
{% extends "base.html" %}
{% block title %}Page Title{% endblock %}
{% block content %}
  ...
{% endblock %}
```

`base.html` is the single source of truth for everything that must appear on every page:
- Global stylesheet (`style.css`)
- Self-hosted HTMX (`htmx.min.js`)
- `IS_ADMIN` JS flag
- Command palette (`palette.js`)
- Site header with nav and Ctrl+K hint
- `hx-boost="true"` on `<body>` for fast navigation

**Exception:** `admin.html` is standalone (no base.html inheritance) because it has a
different header. It must manually include all of the above. When adding a new global
feature, update both `base.html` **and** `admin.html`.

---

## Global JavaScript features

Any JS that injects DOM nodes (like the command palette overlay) must handle two cases:

1. **Initial load** — listen on `DOMContentLoaded`
2. **`hx-boost` navigation** — HTMX replaces `<body>` content without a full page reload,
   so `DOMContentLoaded` does not fire again. Listen on `htmx:afterSwap` as well.
3. **Guard against duplicates** — check if the element already exists before injecting.

```js
function myFeatureInit() {
  if (document.getElementById('my-element')) return; // already present
  // ... inject
}
document.addEventListener('DOMContentLoaded', myFeatureInit);
document.addEventListener('htmx:afterSwap', myFeatureInit);
```

---

## Adding a new section (e.g. a new tracker or tool)

Checklist:

- [ ] Create `templates/<section>/feed.html` extending `base.html`
- [ ] Add route module `src/routes/<section>.rs`
- [ ] Register routes in `src/main.rs`
- [ ] Add DB functions to `src/db.rs`, migration to `run_migrations()`
- [ ] Add nav link to `base.html` header
- [ ] Add palette command to `static/palette.js` COMMANDS array
- [ ] Add CSS under a named section comment in `style.css`

---

## CSS scope

All styles live in `static/style.css`. Sections are marked with comments:

```css
/* ── Section Name ──────────────────────────── */
```

Page-specific styles go under their section heading. Avoid inline `<style>` blocks in
templates except for admin.html (which has its own layout). When a style would be useful
across multiple pages, move it to the global file.

---

## Fitness tracker (Nocturne)

The `/fitness` section is a session-gated, phone-first tracker with its own dark visual
layer — the "Nocturne" design system, scoped under `body.fitness-dark` in `style.css`
(`--noc-*` tokens, `.noc-*` primitives). The rest of the site keeps the light default.

Information architecture (mockups and decisions in `docs/design/fitness-redesign/`):

- **Today** (`/fitness?date=`) — calorie ring + macro rails against editable daily
  targets, a Sunday-first week strip (tap a day to load it), entries grouped into
  breakfast/lunch/dinner/snack slots (clock-inferred default when logging), inline entry
  editing, copy-yesterday, and a sticky Scan · Search · Copy action bar.
- **Add sheet** — full-screen overlay; scanner is the default tab (known barcodes become
  a one-tap log card with portion buttons; unknown ones prefill the add-food form from
  OpenFoodFacts), plus Recent, Favourites, saved Meals and Search tabs.
- **Library** — grouped by category with filter chips; detail form edits per-100g
  values, package/portions, category, favourite and default portion, and shows a 14-day
  usage strip.
- **Week** (`/fitness/week`) — read-only trends: calorie bars vs target, protein
  average/hit-rate, days-logged streak, weight trend with one-tap log, most-logged foods.
- **Desktop** (≥900px) — same column wider, plus a keyboard quick-add (type-ahead;
  Enter logs the food's default portion into the clock-inferred slot).

---

## What not to do

- **Don't duplicate global features** — if something should appear everywhere, it belongs
  in `base.html`, not copy-pasted into each template.
- **Don't put SQL in route handlers** — db.rs owns all queries; routes call db functions.
- **Don't put business logic in templates** — templates receive pre-computed values from
  the route handler; they only format and display.
- **Don't use `DOMContentLoaded` alone** for DOM injection — it won't fire on `hx-boost`
  navigations (see Global JavaScript section above).
- **Don't hardcode image size limits in one place** — the 35 MB upload limit is enforced
  at three layers (nginx, Axum, app). Change all three together or the most restrictive
  one silently wins.
