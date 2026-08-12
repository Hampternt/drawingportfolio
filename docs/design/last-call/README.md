# Handoff: Last Call — game UI

## Overview

Last Call is a party drinking game mode inside the `drinks` crate of the
`Hampternt/drawingportfolio` monolith. Players join a room by code on their phones; an
optional big screen acts as a spectator display. Each player registers what they are
actually drinking, which picks their deck. Cards cost **pulls** (sips) of that drink, so
playing is drinking. Rounds run in six beats and a player is eliminated at 0 HP.

This bundle covers the **game UI**: the phone hand view, the phone table view, and the
1920×1080 big screen. Rules are in the design doc; the reusable pieces and their exact
values are in the module spec.

## About the design files

The `.dc.html` files in this bundle are **design references created in HTML**. They are
prototypes showing intended look and behaviour — they are not production code to lift.
The task is to **recreate these designs in the target codebase's environment** using its
established patterns.

For this project that environment already exists and is unusual, so read this before
starting: the app is **Rust + Axum 0.8, SQLite/sqlx, Askama templates, HTMX with
`hx-boost`, one global stylesheet, no build step and no JS framework**. The `drinks` crate
deliberately does not extend `base.html` and has its own phone-shell layout and its own
stylesheet at `drinkinggame/assets/game.css`. Implement these screens as Askama templates
plus CSS in that crate. Server-push state (round, beat, HP, draws) already flows over SSE
in the existing drinking game modes — reuse that channel rather than adding a second one.

The one place plain HTMX is not enough is the hand wheel (§C.1 below): it needs pointer
drag, momentum snap and per-frame transforms. Write it as a small self-contained
vanilla-JS module in `drinkinggame/assets/`, binding on both `DOMContentLoaded` and
`htmx:afterSwap` and guarding against double-injection — the pattern `static/palette.js`
already uses. No framework, no bundler.

If you are porting this somewhere else entirely, everything below is framework-agnostic;
only the transport notes are specific.

## Fidelity

**High-fidelity.** Every colour, size, font, easing and animation keyframe in the
prototypes is deliberate and is documented here with exact values. Recreate them
faithfully. Where a value here and a value in `Last Call - Game UI.dc.html` disagree, the
HTML is the source of truth.

Two caveats:

- The prototypes are static compositions apart from the hand wheel and the card flights,
  which are live. Interaction states not visible in a still (hover, press, locked, hit,
  eliminated) are specified in prose here and in the module spec, not rendered.
- Card *art* is not designed. The card-art screens in the Game UI file show the upload
  and crop flow, not final artwork.

## Design tokens

### Deck colour — the only taxonomy

Deck identity drives every colour decision in the app. There is no separate card-type
palette and no per-player colour. Two ramps: **fill** for solid areas (cost pips, plaque
top rules, deck dots, card backs) and **ink** for anything sitting on the dark ground
(label text, borders, cost bars). They differ for Wine only, which is too dark to read as
text on near-black.

| Deck   | Fill      | Ink       | Text on fill |
| ------ | --------- | --------- | ------------ |
| Beer   | `#FFB570` | `#FFB570` | `#14101D`    |
| Cider  | `#B48EF7` | `#B48EF7` | `#14101D`    |
| Wine   | `#8B2F4A` | `#D4657F` | `#F2EEF8`    |
| Liquor | `#F7768E` | `#F7768E` | `#14101D`    |
| Soft   | `#6FB6FF` | `#6FB6FF` | `#0D1620`    |

### Surfaces

| Token          | Value     | Use                                     |
| -------------- | --------- | --------------------------------------- |
| page           | `#0B0910` | Document ground, big-screen ground      |
| device         | `#0E0C14` | Phone body                              |
| card / panel   | `#16121F` | Plaques, deck rows                      |
| card alt       | `#17141F` | List rows, secondary buttons            |
| raised card    | `#251F35` | Unfocused wheel cards                   |
| focused card   | `#2E2742` | The focused wheel card                  |
| card back      | `#1B1628` | All card backs and deck stacks          |
| felt           | `radial-gradient(ellipse at 50% 44%, #272038 0%, #191430 52%, #100C1B 100%)` | Table surface |
| felt rail      | `#2A2340` | 11px ring around the felt               |

Hairlines are `rgba(242,238,248,.10)` to `rgba(242,238,248,.28)`. Deck-tinted borders use
the ink hex with an alpha suffix: `59` (subtle), `66` (plaque), `80`–`99` (card back).

### Text

| Token     | Value     | Use                                          |
| --------- | --------- | -------------------------------------------- |
| primary   | `#F2EEF8` | Names, HP, card titles, active tab           |
| body      | `#CDC6DD` | Card body copy, secondary numerals           |
| secondary | `#A79FBB` | Descriptive prose                            |
| label     | `#8D87A0` | Micro-caps, mono metadata, inactive tab      |
| faint     | `#6A6480` | Rail numerals, empty-slot text               |

Status accents: mint `#4FD6A8`, amber `#FFB570`, rose `#F7768E`, azure `#6FB6FF`,
violet `#B48EF7`.

### Typography

Three faces, all already in the repo at `drinkinggame/assets/fonts/` (Archivo and Space
Grotesk are self-hosted woff2; IBM Plex Mono is the system/ui-monospace fallback stack in
the prototypes).

| Role              | Font          | Weight | Size    | Tracking | Notes                        |
| ----------------- | ------------- | ------ | ------- | -------- | ---------------------------- |
| Screen title      | Archivo       | 900    | 26–30px | −0.02em  | Beat name                    |
| Card title        | Archivo       | 900    | 30px    | −0.03em  | On the focused wheel card    |
| Player name       | Archivo       | 900    | 22px    | −0.025em | Plaque                       |
| HP                | Archivo       | 900    | 28px    | −0.03em  | Plaque, largest element      |
| Deck count        | Archivo       | 900    | 22px    | −0.02em  | On deck stacks               |
| Cost pip          | Archivo       | 900    | 17px    | —        | On card faces                |
| Button label      | Archivo       | 900    | 20px    | +0.02em  | Primary actions              |
| Card body         | Space Grotesk | 400    | 15px/1.35 | —      | Card effect text             |
| Prose             | Space Grotesk | 400    | 13px/1.5 | —       | Annotations                  |
| Micro-caps        | Space Grotesk | 700    | 8–11px  | 0.12–0.14em | Uppercase, `#8D87A0`     |
| Machine values    | IBM Plex Mono | 500    | 9–12px  | —        | Time, room code, deck names, counters |

Sans is never uppercased below 700 weight. Mono is never used for paragraphs.

### Shape, elevation, motion

- **Radii:** 3px badges, 5px pips, 6px small cards and secondary buttons, 8px primary
  buttons and panels, 10px plaques, 12–14px card faces, 26px phone bezel, 280px felt.
- **Elevation:** cards use a *hard* offset plus a soft drop —
  `0 3px 0 rgba(5,3,10,.5), 0 8px 16px rgba(5,3,10,.42)` for small cards and
  `0 6px 0 rgba(5,3,10,.6), 0 22px 40px rgba(5,3,10,.55)` for the focused card. The hard
  offset is the deck-of-cards look; do not replace it with a blur-only shadow.
- **Motion:** 130ms taps, 190ms state changes, 280ms position changes, all on
  `cubic-bezier(.2,.8,.3,1)`. Card flights are the exception (see Interactions).
  `prefers-reduced-motion: reduce` must stop the flight animations and hold them at rest
  opacity — the prototypes do this with an attribute selector on the animation name.
- **Spacing:** 4px-derived. Common steps 4, 6, 7, 8, 12, 14, 18, 20, 26, 40.

## Card primitives

One `Card` object, five renderings. The size decides what is dropped; the data shape never
changes. A card the player owns is never shown as a back; a card they do not own is never
shown as a face.

| Rendering    | Size                              | Shows                                                                                                                             |
| ------------ | --------------------------------- | --------------------------------------------------------------------------------------------------------------------------------- |
| **CardFace** | fluid × 176px, pad 16×18, r14      | Deck label (Space Grotesk 700/10px caps, ink, 0.13em) and cost pip on one row; title Archivo 900/30px; body Space Grotesk 400/15px `#CDC6DD`; optional keyword chips (pill, 9.5px caps) |
| **CardPip**  | auto × ~24px, pad 2×11, r5         | Deck fill background, Archivo 900/17px numeral in the deck's text-on-fill colour. Never outlined, never grey                        |
| **CardMini** | 62px wide, pad 7×5, r6             | Cost numeral (Archivo 800/9px, ink) above title (Archivo 800/10px, 1.1 leading, 2-line clamp). 1.5px deck border, hard lift        |
| **CardBack** | 16×24 in hand strips, 44×62 in flight, 46×62 / 68×92 as a pile | Ground `#1B1628`, 1px deck-ink border, and a 9–10px grid pattern at ~10% of the deck hue inset 4–5px. The grid *is* the card back — keep it at every size |
| **CardDot**  | 8×8 circle                        | Deck fill plus `0 0 8px <hue>cc` glow. The smallest legible unit of "a card moved"                                                 |

Card-back grid pattern, exactly:

```css
background-image:
  linear-gradient(<hue>1a 1px, transparent 1px),
  linear-gradient(90deg, <hue>1a 1px, transparent 1px);
background-size: 10px 10px;  /* 9px at 44×62 and smaller */
```

## Screens / views

### 1. Phone shell — 390 × 844

Every phone screen uses the same fixed vertical order. No screen may reorder it.

1. **Status row** — 38–40px, `padding: 0 20px`, mono 12px `#8D87A0`, time left,
   `ROOM 4KQ2` right.
2. **Phase banner** — `padding: 0 18px 12px`, beat name Archivo 900/26px `#F2EEF8`
   uppercase left; `ROUND 6 · BEAT 1 OF 6` in micro-caps right, baseline-aligned.
3. **Tab row** — `padding: 0 18px`, hairline underneath. Three tabs, always in the order
   **HAND / TABLE / LOG**, each `padding: 9px 14px 8px`, Archivo 800/11px, 0.1em tracking.
   Active tab is `#F2EEF8` with a 2px underline; inactive is `#8D87A0` with a transparent
   2px underline so nothing shifts. Underline colour is per-tab, not per-theme: HAND
   `#B48EF7`, TABLE `#6FB6FF`, LOG `#8D87A0`. **The order never changes and the active tab
   is never hoisted to first position** — only colour and underline move.
4. **View** — flexes to fill.
5. **Action bar** — `padding: 12px 14px 18px`. Primary buttons are 64px tall, r8, deck- or
   beat-coloured with `#14101D` label text; a secondary is 92px wide with a
   `rgba(242,238,248,.22)` hairline and `#CDC6DD` text. Two primaries share a row when the
   beat offers a choice, and **the option that involves drinking is always the amber one**.

The thumb zone belongs to the beat's decision. Navigation is at the top precisely because
it is not what you reach for mid-round.

### 2. Phone HAND tab — the hand wheel

The primary interaction and the reason the design exists: it must be operable one-handed,
drunk, without aiming at anything small.

Three parts, always shipped together, laid out left to right inside a
`padding: 14px 12px 0` row:

**a. ArmedColumn — 62px fixed**
Micro-cap header `ARMED n` in violet, then one CardMini per armed card (6px gap), then one
dashed 46px-tall empty slot as an affordance (`1px dashed rgba(242,238,248,.16)`, r6, mono
9px `slot` label). Arming moves a card out of the wheel and into this column; disarming
returns it. The wheel's index list must be genuinely recomputed, not visually filtered.

States: empty (header + one slot) · partial · locked (column drops to 60% opacity, header
reads `LOCKED n`, slot removed).

**b. HandWheel — flexes to fill**
A 3D cylinder of CardFaces rotating on the X axis.

```
STEP    = 21deg per card
RADIUS  = 470px
CARD_H  = 176px

stage:  perspective: 1400px; perspective-origin: 50% 50%; overflow: hidden;
        touch-action: none; cursor: grab
track:  position: absolute; inset: 0; transform-style: preserve-3d;
        transform: translateZ(-470px)
card:   position: absolute; left:0; right:0; top:50%;
        height: 176px; margin-top: -88px;
        transform: rotateX(<-d * STEP>deg) translateZ(470px)
```

Pushing the track back by the radius is what keeps the focused card at its authored size —
without it the perspective projection magnifies it roughly 2×. `d` is the signed distance
in cards from the focus, **wrapped** into `[-N/2, N/2]`.

- **Endless.** The index wraps both ways. There is no first or last card, no end-stop and
  no rubber-band. Hand size can change without the component caring.
- **Drag.** 0.28° of rotation per px of pointer travel. Capture the pointer on
  `pointerdown`; a downward drag moves toward lower index. The entire stage is the drag
  surface.
- **Release.** Snap to the nearest multiple of STEP, animated with a cubic ease-out over
  ~220ms. Mouse wheel and trackpad step exactly one card per notch over ~200ms.
- **Depth.** `opacity: max(0, 1 - 0.48 * |d|)`; `visibility: hidden` past `|d| > 2.05`;
  `z-index: 100 - round(|d| * 10)`.
- **Transitions** are disabled while dragging *and* for any card past `|d| > 1.6`, so a
  card wrapping from one end of the cylinder to the other never animates across the
  screen. Otherwise
  `transform 280ms cubic-bezier(.2,.8,.3,1), opacity 280ms …, background 190ms, border-color 190ms`.
- **Focus styling** applies only at `|d| < 0.5`: `#2E2742` ground, 2px deck-ink border, the
  deep lift. Everything else is a 1px `rgba(242,238,248,.22)` hairline on `#251F35`.

**c. CostRail — 26px fixed**
Down the right edge: the card number in mono 9px `#6A6480` above a flat, right-aligned,
vertically-centred sequence of 3px-tall bars — **one bar per point of card cost**, 2px gap
within a card, 7px gap between cards, coloured by deck ink. The active card's bars are
14px wide at full opacity; all others are 9px at 40%. Transition width and opacity over
190ms.

It is a scrubber, a hand census and a cost histogram in 26px: how many cards you hold, how
expensive they are, which decks they came from, and where you are in the list. Tapping a
group jumps the wheel to that card.

**Demo controls.** The prototype's ↑ / SPIN / ↓ row (44px tall, above the action bar)
exists to demonstrate the motion in a static document. **Do not ship it.**

### 3. Phone TABLE tab — the mini table

For rooms with no big screen. A 466px-tall miniature of the same felt.

- Seats become name/HP chips positioned on the ellipse, top-bordered in the player's deck
  colour.
- Centre column, 112px wide: a 46×62 draw pile (stacked CardBack, remaining count in
  Archivo 900/17px, `DRAW` micro-cap beneath) above a vertical list of deck rows. Each row
  is `#16121F`, r6, `padding: 4px 7px`, containing a 7px deck dot, a micro-cap label and an
  Archivo 900/13px count. The discard row uses a dashed border and a neutral count.
- Card flights are CardDots, not card backs.

Same data, same events, one order of magnitude less detail. If something happens on the
big screen and not here, that is a bug, not a scale decision.

### 4. Big screen — 1920 × 1080

A display, never an input. No hover states, no focus rings, no controls — every affordance
belongs to a phone. Legible from 3–4 metres: nothing under 18px, and HP and the beat name
are the two things visible from the doorway.

**Header — 88px**, `padding: 0 40px`, hairline underneath, `gap: 36px`: the wordmark
`last call` in Archivo 900/28px with a violet full stop; `ROOM` + code and `ROUND` + number
as micro-cap/mono pairs; then pushed right, the beat name in Archivo 900/30px in the beat's
hue and `BEAT 1 OF 6 · 22 CARDS DEALT` in micro-caps.

**Felt** fills everything below. An ellipse inset `40px 26px` from the stage, r280, the
radial gradient above, an 11px `#2A2340` rail, and
`inset 0 0 0 2px rgba(242,238,248,.14), inset 0 50px 110px rgba(5,3,10,.5), 0 26px 80px rgba(5,3,10,.75)`.
A second hairline ellipse sits inset a further 56px.

**Deck row**, centred at (960, 496): five 68×92 DeckStacks plus the discard, 14px gap. Each
stack is a CardBack with one offset shadow card behind it at +3px, the remaining count in
Archivo 900/22px deck ink, and the deck name in micro-caps. The discard has the same
footprint but a dashed hairline, no grid and a neutral `#CDC6DD` count — it is a
destination, not a deck. Under five cards a count turns amber; at zero the stack reads
`RESHUFFLE` and the discard empties into it at the next beat 1.

**PlayerPlaques** are absolutely positioned on the inner ellipse, centred on their own
midpoint, laid out by angle from seat 0 at bottom-centre going clockwise so the local
player is nearest the viewer. Seven seats is the ceiling for one ring; eight compresses the
two bottom positions.

Each plaque is 204px wide, `#16121F`, `1px solid <ink>66`, `border-top: 3px solid <fill>`,
r10, `padding: 11px 14px 12px`, with the small hard lift. Three stacked rows:

1. **Identity** — name Archivo 900/22px left, HP Archivo 900/28px right.
2. **Drinks** — one 8px dot per vessel (3px gap), then the deck names joined by `+` in
   mono 11px `#8D87A0`; the round's draw count right-aligned as a deck-tinted badge
   (`<fill>22` background, r4, Archivo 900/13px — on Wine, solid fill with `#F2EEF8` text).
3. **HandStrip** — separated by a hairline with 9px above and below.

Multi-deck players get one dot per vessel and a 3px top rule split 50/50 between the two
deck fills. Four of the seven seats in the prototype run two drinks; this is normal, not an
edge case.

Plaque states: idle · locked (violet tick beside the name) · drawing (deck rule pulses) ·
hit (HP flashes rose, plaque shakes 4px) · eliminated (whole plaque to 40%, HP replaced by
`GHOST`).

**HandStrip** shows hand size without making anyone read a number: overlapping 16×24
CardBacks at −4px margin, cycling through that player's deck colours so a two-deck hand
reads as two-deck, with the exact count in mono 10px right-aligned.

```
n ≤ 8  →  n backs
n > 8  →  7 backs + "+{n−7}" in Archivo 900/13px
```

## Interactions & behaviour

### Card flights — every card movement in the game

One component, two directions, two scales. A CardBack (big screen, 44×62) or CardDot
(phone, 8×8) is positioned at the source, given the delta to its destination as CSS custom
properties, and animated on a single keyframe track.

```css
@keyframes lc-fly {                     /* big screen, 6s cycle */
  0%   { opacity: 0; transform: translate(0,0) scale(.5); }
  3%   { opacity: 1; transform: translate(0,0) scale(.85); }
  22%  { opacity: 1; transform: translate(var(--dx), var(--dy)) scale(1); }
  27%  { opacity: 0; transform: translate(var(--dx), var(--dy)) scale(.92); }
  100% { opacity: 0; transform: translate(var(--dx), var(--dy)) scale(.92); }
}
@keyframes lc-dot {                     /* phone mini-table, 4.4s cycle */
  0%   { opacity: 0; transform: translate(0,0) scale(.6); }
  8%   { opacity: 1; transform: translate(0,0) scale(1); }
  46%  { opacity: 1; transform: translate(var(--dx), var(--dy)) scale(1); }
  54%  { opacity: 0; transform: translate(var(--dx), var(--dy)) scale(.6); }
  100% { opacity: 0; transform: translate(var(--dx), var(--dy)) scale(.6); }
}
```

- **Draw** = deck pile → plaque, in that deck's colour. Stagger 0.2–0.3s per player so
  seven simultaneous draws read as a burst out of the middle of the table rather than a
  blur. A player on two decks gets one flight per deck.
- **Discard** = plaque → discard slot, same track, later delay. Cards are discarded after
  being played, until the deck is empty.
- Flights live in a single `position: absolute; inset: 0; pointer-events: none` layer above
  the felt.
- **Arrival must tick the destination's counter.** The number and the animation are one
  event, never two.

In production these are one-shot animations driven by SSE events, not the infinite loops
the prototype uses to demonstrate them.

### Phase banner and beat timer

The beat name cross-fades over 280ms on change; the hue does not animate. Each beat owns a
hue: Draw amber, Diplomacy mint, Lock violet, Reveal azure, Resolve rose.

Under the banner sits a 2px rail that fills down over the beat's duration in the beat's
hue, turning rose under 5s. No numerals and no ticking — a bar is legible across a room and
does not create urgency in a game that is meant to be slow.

### Wheel interaction summary

| Input                     | Result                                              |
| ------------------------- | --------------------------------------------------- |
| Drag anywhere on the wheel | Rotate at 0.28°/px, no transitions while dragging  |
| Release                   | Snap to nearest card, ~220ms cubic ease-out         |
| Wheel / trackpad notch    | Step one card, ~200ms                               |
| Tap the focused card      | Arm it — moves to the ArmedColumn                   |
| Tap a card in the column  | Disarm — returns to the wheel                       |
| Tap a rail group          | Jump the wheel to that card                         |
| LOCK                      | Commits the armed set; irreversible                 |

## State management

Per-room state pushed over SSE; the phone posts intents and never advances state itself.

```
room:    code, players[], round, beat, first_seat, rng_seed
player:  seat, name, hp, handicap, vessels[], hand[], armed[], tabs[], status
vessel:  deck, pulls_max, pulls_left, container
card:    id, deck, kind, cost, targets, text, keywords[], duration
play:    card, source_seat, target, paid_from, order_key
effect:  source_play, subject, op, magnitude, expires_round
```

Client-local UI state, not synced: `wheelAngle` (float, degrees), `dragging` (bool),
`activeTab`. The wheel's angle is deliberately local — it is a camera, not game state.

Invariants worth asserting server-side: `0 ≤ pulls_left ≤ pulls_max`; every card in
`hand`/`armed` has a registered vessel of its deck; cards in play + hands + discards =
deck size, per deck; no play survives its own resolution.

The full rule set — the six beats, resolution order, the finish-and-draw economy, the soft
hand cap, elimination — is in `Last Call - Design Doc v2.dc.html`. Read it before building
the beat loop; the numbers still open are tagged `TBD-n` and listed in its §13.

## Assets

- **Fonts** — `drinkinggame/assets/fonts/*.woff2` (Archivo 500–900, Space Grotesk 400–700).
  Already in the repo; the prototypes reference these exact files. Included in this bundle.
- **Icons** — none. The design uses no icon set; everything is type, colour and shape.
- **Card art** — not designed. The card-art screens in the Game UI file specify the upload
  and crop flow only.
- **Design system** — `_ds/hampter-design-system-…/` is included for the token and
  component reference it provides. The drinking game deliberately does not extend
  `base.html`, so treat it as a palette and type reference rather than a stylesheet to
  link.

## Files

| File | What it is |
| ---- | ---------- |
| `Last Call - Game UI.dc.html` | **The source of truth.** All screens: hand wheel (live, draggable), big screen v2, the draw-phase big screen with flights, card art flow, and the phone beat-by-beat screens including the mini table |
| `Last Call - Module Spec.dc.html` | The 20 reusable modules with exact values, grouped A–G, ending in a five-step build order with a done-when for each |
| `Last Call - Design Doc v2.dc.html` | The rules spec: object model, setup, vessel economy, the six-beat round, resolution and timing, hand rules, adjudications, open values, engine notes |
| `Last Call - Game UI v1 archive.dc.html` | Superseded v1 phone table and v1 big screen. Reference only — do not build from it |
| `drinkinggame/assets/fonts/` | The three type faces |
| `_ds/hampter-design-system-…/` | Design-system tokens and components |
| `doc-page.js`, `image-slot.js`, `support.js` | Runtime files the prototypes need in order to open. Not part of the design |

Open any `.dc.html` directly in a browser. The hand wheel is interactive — drag it.

## Still to design

Four screens are not in this bundle and will need design before the feature is complete:
the join/lobby flow, the LOG tab, the end-of-game screen, and card art itself.
