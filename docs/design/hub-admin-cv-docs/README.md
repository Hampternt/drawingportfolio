# Handoff: Hub, Admin, CV and How-it-works — `Hampternt/drawingportfolio`

## Overview

Four screens for the Rust/Axum monolith at `Hampternt/drawingportfolio` (branch `master`):

| Screen | Route | Status in repo |
| --- | --- | --- |
| **Hub** | `GET /` | Exists (`templates/hub/hub.html`) — redesign |
| **Admin** | `GET /admin` | Exists (`templates/admin.html`) — redesign |
| **CV** | `GET /cv` | **New section** |
| **How it works** | `GET /docs` (name it what you like) | **New section** |

The fitness tracker, art portfolio and drinks sections were deliberately left alone.

These four move the site onto the **Hampter Design System** (dark, violet accent) — the same
system `/fitness` and `/artportfolio` already partly use through the `--noc-*` bridge and the
`hm-*` primitives in `static/style.css`.

## About the design files

The `.dc.html` files in this bundle are **design references**, not production code. They are
streaming HTML prototypes: inline styles, a small React runtime (`support.js`) and the design
system's own component bundle (`_ds/`). Do not port them literally.

The task is to **recreate these designs as Askama templates plus CSS in `static/style.css`**,
following the repo's own rules:

- Every user-facing page extends `base.html`. `admin.html` is the standing exception (its own
  header) — when you touch a global feature, update both.
- No `<style>` blocks in templates that extend `base.html`. CSS goes in `static/style.css`
  under a `/* ── Section Name ─── */` comment.
- Every CSS/JS link carries `?v={{ crate::assets::asset_version() }}` — `tests/static_assets.rs`
  fails the build if one forgets.
- Adding a section means updating **all of**: route module + `main.rs` registration, `base.html`
  nav link, a hub card, `static/palette.js` `COMMANDS`, and a `style.css` section.
- Any JS that injects DOM must bind on **both** `DOMContentLoaded` and `htmx:afterSwap`, guarded
  on `e.target === document.body`.

The design system source (tokens, `components.css`, the `hm-*` classes, fonts) is in `_ds/`.
Prefer linking/copying its CSS over re-deriving values: `_ds/…/tokens/*.css` and
`_ds/…/components/components.css` are plain CSS with no build step, which is why they can be
dropped into `static/` as-is. Fonts in `_ds/…/assets/fonts/` are the exact woff2 files the
`drinkinggame` crate already compiles in — serve them from `/static/fonts/`.

## Fidelity

**High fidelity.** Final colours, type, spacing and interaction states. Recreate pixel-for-pixel.
Two caveats:

1. **Icons are Lucide v0.462.0 via CSS mask** (`https://unpkg.com/lucide-static@0.462.0/icons/<name>.svg`),
   masked over `currentColor`. The repo ships no icon set, so this is a flagged substitution —
   self-host the ~20 SVGs used rather than hitting a CDN on every page (the repo self-hosts HTMX
   for exactly this reason).
2. **The mono face is IBM Plex Mono**, also a substitution. Archivo and Space Grotesk are your own files.

## Screens

### 1. Hub — `Hub.dc.html`

**Purpose:** public front door. Portfolio-forward, keyboard-first.

**Layout**, top to bottom, page background `#0B0910`:

- **Sticky header**, 56px tall, `padding: 0 32px`, `background: rgba(14,12,20,.82)` +
  `backdrop-filter: blur(10px)`, `border-bottom: 1px solid rgba(242,238,248,.07)`, `z-index: 40`.
  Flex row, `gap: 24px`.
  - Wordmark `hampter.` — Archivo 800, 17px, `letter-spacing: -0.035em`, `#F2EEF8`, the full stop `#B48EF7`.
  - Nav: Drawing Portfolio · Drawing Tasks · Fitness · Sorting · CV · How it works. 14px/500,
    `#8D87A0`, active `#F2EEF8`, `gap: 20px`, each `white-space: nowrap`.
    **This nav set is identical on Hub, CV and How-it-works — only the active colour differs.**
    It belongs in `base.html`, not per template.
  - Right, `margin-left: auto`, `gap: 12px`: the `Ctrl` `K` keycap button (34px tall, transparent,
    hover `background: rgba(242,238,248,.04)`), then an outline icon-button (`settings`, 34×34)
    shown only when `is_admin`.
- **Hero**, `padding: 96px 32px 72px`, centred 1180px column. Background is the signature
  32px blueprint grid:
  ```css
  background-color: #0B0910;
  background-image:
    linear-gradient(rgba(242,238,248,.045) 1px, transparent 1px),
    linear-gradient(90deg, rgba(242,238,248,.045) 1px, transparent 1px);
  background-size: 32px 32px;
  ```
  - Mono eyebrow, 11px/500, `letter-spacing: .10em`, uppercase, `#5F5876`:
    `rust · axum · sqlite · htmx — one binary, five sections`
  - `h1` `Portfolio.` — Archivo 900, `clamp(44px, 6.4vw, 72px)`, `line-height: 1.04`,
    `letter-spacing: -0.025em`, `#F2EEF8`; full stop `#B48EF7`. `margin-top: 16px`.
  - Tagline, 19px, `#CDC6DD`, `max-width: 56ch` — verbatim from `hub.html`:
    "A collection of personal projects — art, code, and whatever comes next."
  - About line, 16px, `#8D87A0`, `max-width: 62ch`. **Placeholder copy — replace with your own.**
  - **Palette bar**: 48px tall, `max-width: 520px`, `background: #17141F`,
    `border: 1px solid rgba(242,238,248,.10)`, `radius: 8px`, hover border `#B48EF7`.
    `search` icon, the text "Search commands…" (verbatim from `palette.js`'s input placeholder),
    and `Ctrl` `K` keycaps right-aligned. Clicking it opens the palette — same handler as the
    header button.
  - Mono caption under it: "Every destination on this site is one shortcut away".
- **Sections**: heading row (`h2` "Sections", Archivo 700/26px, followed by a 1px
  `rgba(242,238,248,.07)` rule filling the remaining width), then a grid:
  `grid-template-columns: repeat(auto-fit, minmax(min(100%,340px), 1fr)); gap: 16px`.
  Six `HubCard`s — icon in `#B48EF7`, Archivo 600 title, 14px muted description, mono meta line:

  | Icon | Title | Description (verbatim from `hub.html`) | Meta |
  | --- | --- | --- | --- |
  | `image` | Drawing Portfolio | A feed of drawings and sketches. | public feed · admin uploads |
  | `file-text` | CV | Hvor jeg har jobbet, hva jeg har bygget, og det samme som PDF. | one page · pdf download |
  | `list-checks` | Drawing Tasks | Practice prompts on reference images — sorted by subject, difficulty, and task type. | public · filter by subject, difficulty, type |
  | `beer` | Drinks | Party night drink tracker — join with a room code. | 4-letter room code · big screen |
  | `activity` | Fitness Tracker | Track daily meals, calories, and macros. | sign-in required · per-user log |
  | `truck` | Sorting | Crate sort and van load — a pick checklist, a live loading diagram, and the counts checked against each other. | sign-in required · tablet |

  `HubCard` = `.hm-hub` in `components.css`: `background: #17141F`, 1px hairline, 8px radius,
  20px padding, no shadow at rest; hover lifts 2px, border goes `#B48EF7`, adds `--shadow-2`.
- **Footer**: 1px top rule, mono 11px `#5F5876` — wordmark, "how this site works" link,
  "server-rendered · no build step", and `/admin` pushed right.

**Tweakable props in the prototype** (map to template flags, not runtime UI):
`isAdmin` → the existing `is_admin` template bool; `showPaletteBar`; `showTileMeta`.

### 2. Admin — `Admin.dc.html`

**Purpose:** everything currently split between `/admin` and the feed's inline composer, in one
place. Replaces `admin_post_card_html()` — the last consumer of the legacy `.post-card` CSS, so
that block can finally be deleted from `static/style.css`.

**Layout:**

- **Own header** (as today, `admin.html` is standalone): wordmark, a mono `admin` label,
  then right-aligned "View site", the `Ctrl` `K` button, and a small `log-out` Button.
- **Page head** on the blueprint grid, `padding: 44px 32px 32px`: mono
  `signed in as <name> · <role>`, `h1` "Admin" (Archivo 800/40px), and a right-aligned stat row —
  four stacked pairs (mono 22px value over an 11px uppercase label): posts `#F2EEF8`,
  unlisted `#FFB570`, hidden `#F7768E`, accounts `#F2EEF8`. **Sample numbers in the mock;
  wire to `count_posts()` per visibility and the user count.**
- **Body**: `max-width: 1180px`, `grid-template-columns: 236px minmax(0,1fr)`, `gap: 32px`,
  `align-items: start`.
  - **Rail**, `position: sticky; top: 80px`. Groups under mono uppercase labels:
    - *Drawings* → **Posts** (`image` icon, count on the right), **New post** (`upload` icon, `N` keycap)
    - *Owner only* → **Accounts** (`users` icon, count) — rendered only for the owner (`RequireOwner`)
    - A hairline note card (`#0E0C14`) restating the visibility model, with
      `public` `#4FD6A8`, `unlisted` `#FFB570`, `hidden` `#F7768E` inline.
    - Rail item: 34px tall, 5px radius, `gap: 10px`. Inactive `color: #8D87A0`, transparent border.
      Active `background: #262232`, `border: 1px solid rgba(180,142,247,.45)`, `color: #F2EEF8`.
  - **Pane: Posts**
    - Filter row: search `Input` (34px, `search` icon, `/` keycap — the reserved focus key) and
      three interactive `Tag`s: `public` (selected), `unlisted`, `hidden`. These map to the
      admin-only `vis` query param `feed.rs` already parses.
    - Post rows: `#17141F` card, 1px hairline, 8px radius. Inner `padding: 14px 16px`, flex,
      `gap: 16px`:
      - 64×64 thumbnail, 5px radius, on the 16px blueprint grid as its empty state.
        **Real markup must be `<picture>` with AVIF → WebP → original sources and explicit
        `width`/`height`, both or neither** (see `templates/partials/post_card.html`).
      - Caption, Archivo 600/16px `#F2EEF8`; then a mono meta line (11px `#5F5876`,
        `white-space: nowrap` on each item): ISO date · `2480 × 3508` · `2.1 MB` ·
        variant state. Variant state goes amber `#FFB570` while AVIF is still encoding —
        a freshly uploaded post genuinely has an empty `avif_url` for a second or two.
      - Tag chips (`.hm-tag`, pill).
      - Right column: a dotted `Badge` for visibility (`success`/`warning`/`danger` for
        public/unlisted/hidden), and four 28px icon-buttons: `pencil` (edit), `eye-off` (hide),
        `link` (unlist), `trash-2` (delete). Wire to the routes that already exist:
        `PATCH /api/admin/posts/{id}/visibility` (form-encoded `visibility=…`),
        `DELETE /api/admin/posts/{id}`, and `GET /api/admin/posts/{id}/edit`.
        Keep the established swap contract: `hx-target="closest .hm-post" hx-swap="outerHTML"`.
      - **Expanded editor** (the `pencil` toggle): a `#0E0C14` panel inside the same card,
        top hairline, holding a Caption `Textarea`, a Tags `Input` with the hint
        "Comma separated. Saving replaces the whole set.", Save (primary) / Cancel (ghost),
        and a mono `post <id>` on the right. This is `card_edit_popover.html`'s content —
        note the existing reason it is a GET round trip: a `Post` carries no tags, and PATCH
        replaces the whole set.
    - "Load more" secondary Button, centred.
  - **Pane: New post** — one `#17141F` card: `h2` "New post", the line "JPEG, PNG or WebP,
    up to 35 MB." (matches `MAX_IMAGE_BYTES`), a dashed drop zone on the blueprint grid
    (`image-plus` icon, "Click to browse, or drop a file here", mono "no file selected"),
    Caption `Textarea`, Tags `Input`, three visibility `Radio`s (public checked), then a primary
    `upload` Button and the note "Converted to WebP in the browser before it is sent; the AVIF
    variant is encoded after the response and fills itself in." Keep the existing client-side
    canvas → WebP conversion before POST.
  - **Pane: Accounts** (owner only) — intro verbatim from `users.html`, then a table.
    Columns `minmax(140px,1.2fr) 96px 88px 120px minmax(300px,auto)`, `min-width: 780px`
    inside an `overflow-x: auto` wrapper so nothing collides on a narrow window.
    Row: name (Archivo 600/15px, ellipsis), role `Badge` (owner `accent`, admin `info`,
    member `neutral`), PIN state (`set` / `locked` in `#F7768E`), ISO added date, then
    Rename (ghost) · Grant/Revoke admin · Reset PIN · Delete (danger) — the last three
    suppressed for the owner row, matching the `AND is_owner = 0` rule in `db.rs`.
    Below: Name + Initial PIN inputs and a primary `user-plus` "Create account", then the note
    "An admin manages drawings; only the owner can grant admin, reset a PIN or delete an
    account. Five wrong PINs locks the account for 15 minutes."

**Props:** `pane` (`posts` | `new` | `users`), `isOwner`.

### 3. CV — `CV.dc.html`

**Purpose:** a readable CV for an employer, plus a PDF.

**Content is the user's own, verbatim, in Norwegian** — from `uploads/cv-jesper-lovland.html`.
Do not rewrite or translate it. Two fields are still unfilled and are highlighted amber in the
design: `[ARBEIDSGIVER]` and `[ÅRSTALL]` (`color: #FFB570`, `background: rgba(255,181,112,.12)`,
3px radius). **The CV may be swapped for a finished version — keep the content in the template,
not in code.**

**Layout:**

- Same sticky header and nav (CV active).
- Page head on the blueprint grid, `padding: 64px 32px 40px`: mono `cv`, `h1` "Jesper Løvland"
  (Archivo 900, `clamp(38px,5.4vw,60px)`), role line 17px `#CDC6DD`, and right-aligned
  primary "Last ned som PDF" (`download` icon) + secondary "GitHub" Buttons. Below a 1px rule:
  the contact row in mono 12px — `Stavanger, 4042 · jl@dblo.net · +47 405 50 447 ·
  github.com/Hampternt`, separators `#3A3448`, email and GitHub as links, each item `nowrap`.
- Body: `grid-template-columns: minmax(0,1fr) 260px`, `gap: 48px`, `align-items: start`.
  - **Main column**, sections `gap: 44px`. Each section head is a mono 11px uppercase label
    with a 1px bottom rule — `Om meg`, `Prosjekter — bygget selv`, `Arbeidsmåte — AI-assistert
    utvikling`, `Arbeidserfaring`, `Utdanning`.
    Entry: a baseline flex row with `h3` (Archivo 600/20px `#F2EEF8`; the "— qualifier" part
    400 weight `#8D87A0`) and the date right-aligned in mono 12px `#5F5876`; then an optional
    mono meta line and/or `Badge`; then bullets. Bullets are `display: flex; gap: 12px` rows
    with a 4px violet dot (`margin-top: 9px`) — not `list-style`, so drag-reorder and delete
    survive. Prose caps at `68ch`. Entries are separated by 1px hairlines, last one without.
  - **Sticky rail** (`top: 80px`): a `#17141F` card holding the four skill groups (Archivo 600
    12px uppercase heading + 14px `#8D87A0` value), a `#0E0C14` card with the
    Norsk / Engelsk / Førerkort strip, and a link card out to the how-it-works page.

**The PDF button is not wired.** The source CV is already print-ready A4 with `@page` rules —
simplest implementation is to serve that file (or a print stylesheet on this route) rather than
generating a PDF server-side.

### 4. How it works — `How it works.dc.html`

**Purpose:** explain the site to a hiring manager in terms they can follow, and show engineering
judgement while doing it. Written in English (site chrome language) while the CV is Norwegian —
open question for the user.

**Every fact and number comes from the repo's own `CLAUDE.md` / `docs/design.md`.** If you
change the code, change this page: 1105 workspace tests, 458 board checks, 24 migrations,
35 MB upload limit, three-layer limit enforcement, 39s → 3s upload, one-year immutable static cache.

**Sections in order:**

1. Page head on the blueprint grid: mono `documentation · written for a reader who does not code`,
   `h1` "How this site works." (violet stop), an 18px lede at `68ch`, then a four-stat mono row —
   program `1`, sections `5`, automated tests `1105` (`#4FD6A8`), build step `none`.
2. **What it is** — two `68ch` paragraphs, plus an optional "For a technical reader" aside:
   `#0E0C14`, 1px hairline, `border-left: 2px solid #B48EF7`, radius `0 8px 8px 0`. The asides
   are behind the `showDetail` prop (default off).
3. **What happens when you open a page** — four boxes in
   `repeat(auto-fit, minmax(min(100%,210px),1fr))` on a `#0E0C14` panel. Steps 2–4 carry a violet
   `→` inside their own mono label so nothing dangles when the row wraps; step 4 has the accent
   border (the one accent-bordered element in the group). Content: nginx front door → the Rust
   program → SQLite + object storage → filled template as finished HTML.
4. **The five sections** — a table (`150px minmax(260px,1fr) 190px`, `min-width: 640px` inside
   `overflow-x: auto`): name + mono route, what it does, who can use it.
5. **A worked example: uploading a drawing** — three timeline rows (in browser `< 1 s` /
   in request `2–3 s` in `#4FD6A8` / after, "nobody waits" in `#FFB570`), then the rule it
   generalises to.
6. **Who can see what** — four cards in `repeat(auto-fit, minmax(min(100%,340px),1fr))`, each a
   `Badge` (visitor `neutral`, member `info`, admin `accent`, owner `warning` + accent border)
   over one sentence. **Badge mounts must not stretch:** wrap each in `<span style="display:flex">`
   or give the card `align-items: flex-start`. Optional technical aside covers the four session
   extractors, 404-not-403, the required-viewer compile-time guard, and PIN lockout.
7. **How I know it still works** — three cards: `1105 + 458` automated checks, `2 gates`
   (the ~6s check and the full verify run), and "tried in a browser".
8. **Decisions I would defend** — five choice/cost rows: server-rendered HTML over a framework;
   unlisted is not secret (sequential ids); the UTC day boundary; the one-year static cache and
   its `?v=` bust; one dark theme.
9. **Keyboard callout** on the blueprint grid + a mono footer (wordmark, GitHub source link, cv link).

**Props:** `showDiagrams` (default on), `showDetail` (default off).

## Interactions & behaviour

- **Command palette** — `Ctrl`/`⌘` `K` toggles, `Esc` closes, `↑` `↓` move, `↵` runs.
  The prototypes reimplement `static/palette.js`; **keep the real one**, and add the new commands:
  `Go to CV` (keywords: cv resume work experience hire pdf) and `How this site works`
  (docs documentation architecture stack). The palette bar in the hub hero and the header keycap
  button both dispatch the same synthetic keydown `admin.html`/`base.html` already use.
- **Admin panes** — rail selection. Prototype holds it in local state; server-side, either
  three routes or one route with a query param. Don't lose `hx-boost`.
- **Post edit** — the `pencil` icon toggles the inline editor; open one at a time.
- **Hover** — cards lift 2px + `--shadow-2` and take an accent border; controls add 4% white or
  step the accent one stop lighter.
- **Press** — `translateY(1px)` everywhere. Never a scale.
- **Focus** — the same double ring on everything: `0 0 0 2px #0B0910, 0 0 0 4px #B48EF7`.
  Never removed, never restyled per component.
- **Motion** — 130ms controls, 190ms surfaces, 280ms overlays, all
  `cubic-bezier(.2,.8,.3,1)`. Every transition needs a `prefers-reduced-motion` branch.
- **Responsive** — no fixed widths on text containers. The 1180px page centres with 32px gutters;
  the 236px admin rail and the 260px CV rail sit above their content on a narrow window; tables
  scroll horizontally rather than colliding.

## State

| Screen | State | Notes |
| --- | --- | --- |
| Hub | palette open | Global, already in `palette.js` |
| Admin | active pane; editing post id; search text; visibility filter set | Filter maps to the existing `PostFilter` (`q`/`tags`/`collection`/`vis`) — `vis` is admin-only and silently dropped otherwise |
| CV | palette open | Static page |
| How it works | palette open; `showDiagrams`; `showDetail` | The two flags can be template constants |

## Design tokens

Authoritative source: `_ds/hampter-design-system-…/tokens/*.css`. Used here:

**Ink ramp** — `#0B0910` page · `#0E0C14` raised · `#17141F` card · `#262232` chip ·
`#3A3448` hairline-strong · `#5F5876` faint text · `#8D87A0` muted text · `#CDC6DD` body ·
`#F2EEF8` strong text. Borders: `rgba(242,238,248,.07)` subtle, `rgba(242,238,248,.10)` default,
`rgba(180,142,247,.45)` accent.

**Accent** — violet `#B48EF7` (primary button, focus ring, links, active underline, palette
cursor, the wordmark's full stop — nothing else). Warm amber `#FFB570` is the secondary
highlight. Status: mint `#4FD6A8`, amber `#FFB570`, rose `#F7768E`, azure `#7AA2F7`.

**Type** — Archivo 800/900 headings and wordmark, `letter-spacing: -0.025em` (wordmark
`-0.035em`), leading 1.04–1.22 · Space Grotesk 400/500 prose and controls at 15px/1.5 ·
IBM Plex Mono for dates, sizes, counts, keycaps and the 11px uppercase micro-labels
(`letter-spacing: .10em`). Sans is never uppercased; mono is never used for paragraphs.

**Spacing** — 2, 4, 6, 8, 12, 16, 20, 24, 32, 40, 48, 64, 80, 112, 160. Controls snap to
28 / 34 / 42px. Page 1180px, gutters 32px, rails 236px (admin) / 260px (CV), header 56px,
sticky rails `top: 80px`, prose `68ch`.

**Radius** — 3px keycaps and badges · 5px buttons, inputs, thumbnails · 8px cards ·
12px dialogs and the palette · 18px large panels · pill for tags.

**Shadow** — none at rest. `--shadow-2` on card hover, `--shadow-3` for the palette, dialogs
and toasts. Transparency and blur appear exactly twice: the sticky header
(`rgba(14,12,20,.82)` + 10px blur) and the modal scrim (72% black + 3px blur).

**Blueprint grid** — the one background treatment, 32px at 4.5% white (16px inside empty
media slots). No photography, no illustration, no gradients.

## Assets

- **Fonts** — `_ds/…/assets/fonts/*.woff2`: Archivo 500–900, Space Grotesk 400–700. These are the
  same files `drinkinggame` already ships. Serve from `/static/fonts/` and preload **per page**,
  only the faces above the fold (e.g. `archivo-800` + `space-grotesk-400`).
- **Mono** — IBM Plex Mono, currently Google Fonts. Self-host it or swap it.
- **Icons** — Lucide v0.462.0, CSS-masked. Names used: `image`, `file-text`, `list-checks`,
  `beer`, `activity`, `truck`, `home`, `upload`, `eye`, `eye-off`, `link`, `tag`, `search`,
  `settings`, `users`, `user-plus`, `pencil`, `trash-2`, `image-plus`, `book-open`,
  `calendar-days`, `clipboard-list`, `log-out`, `chevron-down`, `download`, `github`.
- **No logo exists** and none was invented: wherever a mark belongs, the wordmark `hampter.` is set
  in Archivo Black, lowercase, `-0.035em`, with a violet full stop.
- No drawings are bundled. Image slots in the mocks are the blueprint-grid empty state.

## Files in this bundle

| File | What |
| --- | --- |
| `Hub.dc.html` | New hub design |
| `Admin.dc.html` | New admin design |
| `CV.dc.html` | New CV page |
| `How it works.dc.html` | New documentation page |
| `Current Hub.dc.html` | Pixel recreation of today's `/` — for before/after comparison |
| `Current Admin.dc.html` | Pixel recreation of today's `/admin` |
| `_ds/` | The design system: tokens, `components.css`, the component bundle, fonts |
| `support.js` | Runtime the prototypes need to render. Not for production |
| `github.md` | Repo association, sync record, screen → source map |

Open any `.dc.html` directly in a browser. The `_ds/` paths inside them are relative to the
bundle root, so keep the folder structure.

## Open items the user still owns

1. `[ARBEIDSGIVER]` and `[ÅRSTALL]` in the CV are unfilled; the CV may be replaced wholesale.
2. The CV's PDF button is not wired.
3. The docs page is English while the CV is Norwegian.
4. Admin header stats and post rows are sample data.
5. The hub's about line is placeholder copy.
