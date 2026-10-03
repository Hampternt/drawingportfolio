# Hampter Design System

A dark-native design system for **hampter's personal site** — the Rust/Axum monolith in
[`Hampternt/drawingportfolio`](https://github.com/Hampternt/drawingportfolio) (branch
`master`). The site works; its visual layer grew section by section and no longer
coordinates. This system is the target to migrate toward, gradually, one section at a time.

## Context

The site is a single Rust binary serving four surfaces behind one header:

| Surface | Route | What it is |
| --- | --- | --- |
| **Hub** | `/` | Index page — four section tiles. |
| **Drawing Portfolio** | `/artportfolio` | Public feed of drawings; admin-only uploads (35 MB, JPEG/PNG/WebP, WebP + AVIF variants generated on upload). |
| **Drawing Tasks** | `/tasks` | Practice prompts on reference images, filtered by subject, difficulty and task type. |
| **Fitness Tracker** | `/fitness` | Session-gated nutrition tracker: calorie ring, macro rails, week strip, meal slots, barcode scanning. Has its own dark theme today ("Nocturne"). |
| **Drinks** | `/drinks` | Separate crate: a party drinking game (Ring of Fire, 3 Man) with a three-tab phone shell, SSE leaderboards and a spectator "big screen". Deliberately does not extend `base.html`. |

**Stack:** Rust + Axum 0.8 · SQLite (sqlx) · Askama templates · HTMX with `hx-boost` ·
S3-compatible object storage · WebAuthn passkeys. Server-rendered HTML, one global
stylesheet (`static/style.css`), no build step, no framework.

**The signature interaction already exists:** `static/palette.js` puts a `Ctrl+K` command
palette on every page, injected on both `DOMContentLoaded` and `htmx:afterSwap`, with
arrow-key navigation and an admin-gated command set. The header carries a live
`Ctrl` `K` button. This system treats that as the primary navigation, not a nicety.

### Sources read

- Repo: `Hampternt/drawingportfolio` @ `master` — see `github.md` for the sync record and
  the screen → file map.
- `CLAUDE.md`, `docs/design.md` — architecture and the site's own design rules.
- `static/style.css` (`body.fitness-dark`, `--noc-*` "Nocturne" tokens), `static/palette.js`.
- `drinkinggame/assets/game.css` — the newest and most deliberate dark palette; the
  colour system here is built from it.
- `drinkinggame/assets/fonts/*.woff2` — Archivo and Space Grotesk, copied into `assets/fonts/`.
- Templates: `base.html`, `hub/hub.html`, `artportfolio/feed.html`, `tasks/feed.html`,
  `fitness/week.html`, `partials/post_card.html`, `login.html`.

---

## Content fundamentals

**Voice.** Plain, first person, understated — the repo's own register. Descriptions are
one line and factual: *"A feed of drawings and sketches."* · *"Practice prompts on
reference images — sorted by subject, difficulty, and task type."* · *"Party night drink
tracker — join with a room code."* Nothing is sold; things are named.

**Person.** "I" for anything autobiographical, "you" only in instructions. Never "we".

**Casing.** Sentence case for headings, buttons and nav labels ("Drawing Tasks", "New
post", "Load more"). Lowercase for tags, meal slots and terminal output (`breakfast`,
`figure`, `timed`). UPPERCASE only for the 11px mono micro-labels, tracked to 0.10em.

**Length.** Section descriptions are one sentence. Prose stops at 68ch. Toasts are one
line. Empty states are a single mono line behind a `>` prompt.

**Numbers and metadata** are mono and exact — `1840 / 2100 kcal`, `9 days`, `128 posts`,
`2026-07-21`. Dates are ISO. Units are lowercase and attached (`250 g`, `35 MB`).

**Errors** say what happened and the limit that was hit: *"File is 41 MB — the limit is
35 MB."* Never "Something went wrong".

**Emoji: never.** Status is a `Badge`, taxonomy is a `Tag`, everything visual is a Lucide glyph.

---

## Visual foundations

**Colour.** One dark theme; no light mode. The ink ramp is a violet-tinted near-black
lifted straight from the drinking game — `--ink-950 #0B0910` (page), `--ink-900 #0E0C14`
(raised), `--ink-850 #17141F` (cards), `--ink-700 #262232` (chips), up to `--ink-050
#F2EEF8` (text). The brand hue is **violet `--violet-400 #B48EF7`**, used for the primary
button, focus rings, links, the active tab underline, the palette cursor and the full
stop in the wordmark — nothing else. A warm **amber `#FFB570`** is the secondary
highlight (streaks, live state, numbers worth noticing). Status: mint `#4FD6A8`, amber
`#FFB570`, rose `#F7768E`, azure `#7AA2F7`.

**Type.** Two self-hosted faces plus one mono. **Archivo** (800/900) for headings and the
wordmark, tracking −0.025em, leading 1.04–1.22. **Space Grotesk** (400/500) for prose and
controls at 15px/1.5. **IBM Plex Mono** for anything a machine produced: dates, weights,
calorie counts, keycaps, code, the `>` prompt, and 11px uppercase micro-caps. Sans is
never uppercased; mono is never used for paragraphs.

**Spacing and layout.** A 4px-derived scale with fine steps at the bottom (2, 4, 6, 8, 12,
16, 20, 24, 32, 40, 48, 64, 80, 112, 160). Controls snap to 28 / 34 / 42px. Layout is a
centred 1180px page with 24px gutters, a 236px filter rail, a 56px sticky header and a
68ch prose measure. Sticky: the header (translucent + blurred) and side rails at
`top: 80px`. Nothing else moves.

**Backgrounds.** No photography, no illustration, no gradient meshes. The signature
backdrop is a 32px blueprint grid at ~4.5% white behind heroes, page heads and empty
media slots. Drawings supply all the colour a page needs.

**Borders, cards and elevation.** A border separates; a shadow only suggests depth. Cards
are `--surface-card #17141F` with a 1px hairline and an 8px radius, no shadow at rest.
Hover adds `--shadow-2` and lifts 2px. Dialogs, the palette and toasts sit on
`--shadow-3`. One accent-bordered card per screen, maximum.

**Corner radii** are small: 3px keycaps and badges, 5px buttons and inputs, 8px cards,
12px dialogs and the palette, 18px large panels, pill only for tags and rails.

**Interaction states.** Hover adds 4% white, or steps the accent one stop lighter
(400 → 300) on filled controls. Press steps one stop darker (400 → 500) and translates
1px down — `translateY(1px)` everywhere, never a scale. Focus is always the same double
ring: 2px page colour then 2px violet. Disabled is 42% opacity, pointer events off.

**Motion.** 130ms controls, 190ms surfaces, 280ms overlays, all on
`cubic-bezier(.2,.8,.3,1)`. Dialogs and the palette rise 8px while fading; toasts slide
12px from the right; rails and rings animate their fill over 280ms. No bounce, no
parallax, no scroll reveals. `prefers-reduced-motion` zeroes every duration token.

**Transparency and blur** appear twice only: the sticky header (`rgba(14,12,20,.82)` +
10px blur) and the modal scrim (72% black + 3px blur). Cards are never translucent.

**Imagery.** Drawings are shown uncropped at their natural aspect ratio in a column grid —
never force-cropped to a tile. No filters, no tints: the artwork is the only saturated
thing on the page.

---

## Iconography

**Lucide** (v0.462.0) is the icon set, loaded from CDN. The repo ships no icon font,
sprite sheet or SVG set — it uses text labels and a couple of unicode characters — so
this is a **flagged substitution**: Lucide's 1.5px stroke suits the hairline aesthetic,
but swap it freely; only the `Icon` component changes.

Icons render as a **CSS mask** over `currentColor`
(`https://unpkg.com/lucide-static@0.462.0/icons/<name>.svg`), so a glyph always inherits
its container's colour. Sizes: 14px in `sm` controls, 16px default, 18–20px standalone.
Never inline a hand-drawn SVG, never use an emoji as an icon.

Unicode is used for one thing: **keycaps** — `Ctrl` `K`, `↑` `↓` `↵` `Esc` — set in mono
inside `Kbd`, matching the `<kbd>` elements already in `base.html`.

**There is no logo.** The repo contains no brand mark and none was invented. Wherever a
mark would go, set the name in Archivo Black, lowercase, tracking −0.035em, with a violet
full stop: `hampter.` (see `guidelines/brand-wordmark.html`). Supply a real logo and it
replaces every wordmark instance.

---

## Migrating from what's live

1. Link `styles.css`, then delete rules from `static/style.css` section by section — the
   token names are new, the CSS is plain, and nothing here needs a build step.
2. `tokens/legacy-nocturne.css` re-points every `--noc-*` variable at the new tokens, so
   the fitness tracker keeps rendering while its markup is converted. Two deliberate
   breaks: the accent moves from `#9184d9` to `#B48EF7`, and the 1px-ring "shadow"
   becomes a real border.
3. Fonts are already yours — `assets/fonts/*.woff2` are the exact files the drinking game
   compiles in. Serve them from `/static/fonts/` and keep the `@font-face` block.
4. The drinking game is closest to this system already; the fitness tracker is second;
   the hub, art feed and tasks board are the ones that change most.

---

## Keyboard

The site is keyboard-first. That is a discipline, not a feature: every destination is
reachable without a mouse, every shortcut is visible somewhere on screen, and the key map
is fixed across sections.

**Reserved keys.** These mean the same thing on every page and a section may never
rebind them:

| Key | Action |
| --- | --- |
| `Ctrl` `K` / `⌘` `K` | Open or close the command palette |
| `?` | Show the keyboard shortcuts overlay |
| `/` | Focus this page's search or filter field |
| `Esc` | Close the topmost overlay, or blur the focused field |
| `↑` `↓` | Move the selection inside an overlay |
| `↵` | Run or open the selection |
| `K` `J` | Previous / next item in a list |

Single-letter shortcuts never fire while an `input`, `textarea` or `select` has focus.
Anything a section adds is a single unmodified letter, listed in the overlay, and free of
the reserved set.

**The palette is the navigation.** Adding a section to the site means adding, together: a
route, a nav link, a `HubCard`, and a palette command. A destination that is not in the
palette does not exist. Commands are grouped under short imperative headings (Navigate,
Admin, Theme); `adminOnly` commands are hidden unless the palette is passed `isAdmin`,
matching the `IS_ADMIN` flag `base.html` already sets.

**Focus rules.**
- Focus is always visible: the same double ring (2px page colour, 2px violet) on every
  control, never removed, never restyled per component.
- Opening an overlay moves focus into it — the palette's input, the dialog's first
  control. Closing returns focus to whatever opened it. `CommandPalette` does this already.
- Overlays trap focus while open; the page behind them is inert.
- Tab order follows reading order. No positive `tabindex`, ever.
- Any element with a click handler is either a `button`/`a` or carries `tabIndex={0}` plus
  a key handler — `Card interactive` does this.

**Showing shortcuts.** A key is discoverable in at least one of three places, in this order
of preference: on the control itself (`Button shortcut`, the header's `Ctrl` `K` button),
in its tooltip (`Tooltip shortcut`), or in the `?` overlay. The overlay is the complete
list — if a shortcut is not in `ShortcutsOverlay`, it is not a shortcut. Keycaps use real
glyphs (`⌘ ⇧ ⌥ ↵ ↑ ↓ Esc`) set in mono inside `Kbd`.

**HTMX caveat.** `hx-boost` swaps body children without firing `DOMContentLoaded`, so any
global key handler must bind on both `DOMContentLoaded` and `htmx:afterSwap` and guard
against double-injection — the pattern `static/palette.js` already uses. Keep it when
porting these components back into the Askama templates.

---

## Index

| Path | What |
| --- | --- |
| `styles.css` | The entry point consumers link. `@import`s only. |
| `tokens/` | `fonts.css`, `colors.css`, `typography.css`, `spacing.css`, `shape.css`, `motion.css`, `base.css`, `legacy-nocturne.css`. |
| `assets/fonts/` | Archivo 500–900 and Space Grotesk 400–700 woff2, from the repo. |
| `components/components.css` | Every component's CSS, keyed to the tokens. |
| `components/core/` | `Button`, `IconButton`, `Icon`, `Kbd` (+ `KbdGroup`), `Badge`, `Tag`, `Card`, `HubCard`, `PostCard`. |
| `components/forms/` | `Input` (+ `Textarea`), `Select`, `Checkbox`, `Radio`, `Switch`. |
| `components/navigation/` | `Tabs`, `CommandPalette`, `ShortcutsOverlay` (+ `RESERVED_SHORTCUTS`). |
| `components/feedback/` | `Dialog`, `Toast` (+ `ToastStack`), `Tooltip`. |
| `components/data/` | `CalorieRing`, `MacroRail`. |
| `guidelines/` | 21 specimen cards: colour, type, spacing, shape, motion, brand motifs, the reserved key map. |
| `ui_kits/portfolio/` | Click-through kit: hub, art feed, drawing tasks, fitness Today, Ctrl+K palette. |
| `templates/` | Three starting templates: **Hub page**, **Feed page**, **Filtered list page**. |
| `github.md` | Repo association, last sync, screen → source map. |
| `SKILL.md` | Agent-skill wrapper for use outside this project. |

Every component has a sibling `.d.ts` (props contract) and `.prompt.md` (when to use it,
with an example). Read the `.prompt.md` before using a component.

### Intentional additions

The repo defines no component library — it is server-rendered HTML with a global
stylesheet — so the standard primitive set was authored. These are the ones that exist
because *this* site needs them:

- **`Kbd` / `KbdGroup`** — the site is keyboard-first; `base.html` already renders `<kbd>`
  elements in the header.
- **`CommandPalette`** — a component form of `static/palette.js`, same keys, same behaviour,
  same `adminOnly` gating.
- **`ShortcutsOverlay`** — the `?` overlay, and the canonical list of reserved keys
  (`RESERVED_SHORTCUTS`). The repo has no equivalent yet; a keyboard-first site needs one.
- **`HubCard`** — the index page is nothing but these (`hub.html`).
- **`PostCard`** — the art feed unit (`post_card_html()`).
- **`CalorieRing` / `MacroRail`** — the fitness tracker's two data primitives.
- **`Icon`** — a thin Lucide wrapper so glyphs inherit `currentColor`.

### Known substitutions — please confirm

1. **Mono face.** Archivo and Space Grotesk are your own files; the mono is IBM Plex Mono
   from Google Fonts, because the repo self-hosts no monospace face. Say the word and it
   can be swapped or self-hosted.
2. **Icons.** Lucide, from CDN. Substituted, not sourced.
3. **Logo.** Absent by design — see Iconography.
4. **`/drinks`.** Not recreated in the UI kit; it has its own phone-shell layout and is
   the surface that already matches this palette.
