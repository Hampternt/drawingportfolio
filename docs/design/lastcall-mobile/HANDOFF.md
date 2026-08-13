# Handoff: Last Call — Mobile Play Flow

## Overview
A redesigned phone experience for the **Last Call** drinking card game (the `drinkinggame` crate in `Hampternt/drawingportfolio`, served at `/drinks`). It restructures how a player reads, plays, mulligans and resolves cards on a phone:

- **HAND tab = reading.** A 3D spinning wheel of full card faces (as in the live `lc_wheel.js`), plus an inspect sheet with full rules text.
- **TABLE tab = playing.** Cards are played from a fanned tray at the bottom of the felt: tap or drag a card → a full-pane target overlay with large "box list" drop zones → the card flashes large in the felt centre, then flies into an **ARMED stack** on the felt's left edge with a curved dotted arrow to its target. Any number of cards can be armed; each is removable ("take back") until **LOCK IN** resolves all of them at once (the game's reveal).
- **Mulligan** (once per game): a Gwent-style full-screen multi-select — swap any number of cards, up to the whole hand.
- A compact **mode badge** in the tab bar always states what the player is doing (READ / PLAY / TARGET / ARMING / MULLIGAN), with the player's pull count beside it.

## About the Design Files
The file in this bundle (`Last Call Mobile.dc.html`) is a **design reference created in HTML** — an interactive prototype showing intended look and behavior, not production code. The task is to **recreate this design in the existing codebase**: the Rust/Axum server-rendered shell (`lc_room.html`, `lc_render.rs`, `lastcall.css`, `lc_wheel.js`, `lc_loop.js`). The prototype was deliberately built from that codebase's own tokens, class anatomy and card catalog, so almost every value below maps 1:1 onto `lastcall.css`.

The prototype's game logic (immediate HP changes, local queue) is demo-only; the real engine's arm → lock → reveal flow is the source of truth.

## Fidelity
**High-fidelity.** Colors, type, spacing and card anatomy match `drinkinggame/assets/lastcall.css` exactly (tokens listed below). Implement pixel-perfectly with the existing CSS conventions (extend `lastcall.css`; renderers emit deck class names, never hex).

## Screens / Views

### Shell (all tabs)
Fixed vertical order (matches the existing F.1 phone shell): status row → beat banner → tab row → view → note row → action bar.
- **Status row**: 34px, mono 11px `#8D87A0`, clock left, "ROOM K7Q2" right, `white-space:nowrap`.
- **Banner**: beat name (Archivo 900 26px uppercase, beat hue — PLAY is violet `#B48EF7`) left; meta "ROUND 3 · BEAT 3 OF 5" (Space Grotesk 700 10px, letter-spacing .13em, uppercase, `#8D87A0`, nowrap) right. Padding 0 18px 9px.
- **Tab row**: HAND / TABLE / LOG buttons (Archivo 800 11px, .1em; selected `#F2EEF8` with 2px underline — hand violet `#B48EF7`, table azure `#6FB6FF`, log `#8D87A0`; unselected `#8D87A0`). Right-aligned in the same row: **mode badge** (pill, Archivo 800 8.5px .13em; background = mode ink at ~13% alpha (`ink + '22'`), border ink at ~33% (`ink + '55'`), text = mode ink) and **pull count** ("9 PULLS", mono 10px amber `#FFB570`). Badge states: READ (violet) on hand, PLAY (azure) on table, TARGET (azure) while a card is staged/dragged, ARMING (card's deck ink) during the arm flash, MULLIGAN (violet) in the swap overlay, LOG (grey).
- **Note row** (until mulligan used): "MULLIGAN AVAILABLE · SWAP ANY NUMBER OF CARDS, ONCE" — mono 10.5px `#8D87A0`, centered.
- **Action bar**: padding 10px 14px 16px, gap 10. `MULLIGAN` secondary (118px, transparent, 1px border rgba(180,142,247,.55), text `#B48EF7`, hidden after use) + `LOCK IN` primary (flex 1, 58px, radius 8, violet `#B48EF7` bg, `#14101D` text, Archivo 900 18px). With armed plays the label becomes "LOCK IN · n QUEUED". There is **no per-card confirm bar** — arming is confirm-free by design.
- The prototype presents inside a 390×844 rounded frame (radius 30, hairline border) centered on `#0B0910`; in production the shell is simply the page.

### HAND tab (read mode)
- **Hand wheel**: the existing 3D cylinder (STEP 21°, RADIUS 470px, perspective 1400px). Card wrappers: left/right 20px, height 176px, centered vertically; transform `rotateX(-d*21deg) translateZ(470px)`; opacity 1 focused, fading by `max(0, 1-(|d|-.5)*.42)`; hidden past |d|>3.4. Transitions 280ms `cubic-bezier(.2,.8,.3,1)`, none while dragging. **Important:** the wheel stage needs `clip-path:inset(0)` in addition to `overflow:hidden` — 3D (`preserve-3d`) descendants are not clipped by overflow alone.
- **Card face** (from `card_face()` / `.lc-cardface`): raised `#251F35`, 1px `rgba(242,238,248,.22)`, radius 14, padding ~14px 17px, shadow `0 3px 0 rgba(5,3,10,.5), 0 8px 16px rgba(5,3,10,.42)`. Deck label 10px 700 .13em uppercase in deck ink; duration ("2 ROUNDS") mono 8.5px `#6A6480`; cost pip (deck fill bg, on-fill text, Archivo 900 16px, radius 5, padding 2px 11px); title Archivo 900, ramp 26/21/18px by length (≤14 / ≤24 / more chars), -.03em, lh 1.05; body 13.5px lh 1.35 `#CDC6DD`, 3-line clamp; keyword chips (pill, 9px 700 .12em uppercase, deck ink, hairline border).
- **Focused-card affordances**: focused card gets `#2E2742` bg + 2px deck-ink border + a right-aligned mono "TAP TO READ" hint (8.5px, nowrap) in the keyword row; position counter "03 / 07" top-right of the stage (mono 10px, z above cards, nowrap); pulsing hint line **below** the stage (never inside it): "DRAG UP OR DOWN TO SPIN · TAP THE FRONT CARD TO READ" (9px 700 .14em `#6A6480`).
- **Interaction**: vertical drag spins (≈0.016 cards/px, snap to nearest on release, rubber-band at ends); tap (<10px movement) opens the inspect sheet for the focused card. No separate READ/PLAY buttons.
- **Inspect sheet** (bottom sheet, 240ms rise): dim scrim rgba(11,9,16,.78) + blur 2px (tap closes); sheet `#16121F`, top radius 18, hairline top border; grabber pill; expanded card face (no clamps, all keywords, 26px title, effect line in ink e.g. "5 DAMAGE"); 2×2 meta grid of `#17141F` cells (label 8.5px 700 .14em `#6A6480`; value mono 12.5px): TARGETS (ONE PLAYER / EVERYONE / YOURSELF), PULL COST, DURATION (or IMMEDIATE), DECK; buttons CLOSE (secondary 92px) + "PLAY ON THE TABLE →" ("ARM ON THE TABLE →" for reactions) in deck fill/on-fill — stages the card and switches to TABLE.
- **Side-quest drawer** ("YOUR TAB"): out of the way by default — a vertical-text handle ("YOUR TAB ◂", Archivo 800 9px .16em violet, radius 8 0 0 8, `#251F35`) docked on the right edge near the bottom (bottom ~34px). Tap slides a 212px panel out (translateX 212px→0, 280ms): kicker "YOUR TAB — SIDE QUEST", name "SHOWBOAT" (Archivo 900 18px), rule text 12.5px `#8D87A0`, "PAYS +2 PULLS" mono 11px.

### TABLE tab (play mode)
- **Felt**: fills the space above the tray; radius 170px, 9px rail border `#2A2340`, radial gradient `#272038 → #191430 52% → #100C1B`, inset ring highlights, inner hairline ellipse at inset 32px.
- **Seat chips** on the ring — opponents NORA (24%,18%), VIKTOR (50%,11%), EMIL (76%,18%); you (MAJA) at (50%,84%). Chip: `#16121F`, 1px deck-ink-at-40% border, radius 10, min-width 78, shadow lift; name Archivo 800 11px + violet "YOU" tag on self; HP Archivo 900 16px (flashes rose `#F7768E` on hit / mint `#4FD6A8` on heal-shield, with a 320ms shake on hit); **hand-composition strip**: per deck in hand, a 9×13px card-back swatch (deck ink at ~18% bg, 1px ink border, radius 2) + count in ink (Archivo 800 9.5px) — e.g. wine▮4 soft▮2. Your own strip derives live from your hand.
- **Centre (idle)**: 46×62 grid-textured card back (`#1B1628`, 10px grid lines at 6% white) + "PLAYS n" mono 9px.
- **Tray** (bottom of pane): header "YOUR HAND · n" / "DRAG A CARD ONTO A PLAYER" (9px 700 .14em `#6A6480`, nowrap). Cards are **fanned minis** (64px wide, overlapping `margin-left:-14px`, padding-left 16px compensation): cost (Archivo 800 9px ink) + 2-line title, `#17141F` bg, 1.5px ink border, `touch-action:none`. No visible scrollbar ever (`scrollbar-width:none` + `::-webkit-scrollbar{display:none}` — the one legitimate stylesheet rule). Staged card lifts -6px with 2px border and `#2E2742` bg.
- **Targeting overlay** (appears when a card is staged by tap OR mid-drag): covers the **whole table pane** (not just the felt), rgba(11,9,16,.82), hidden scrollbars. Label "CHOOSE A TARGET — TAP OR DROP" (9px azure). **Drop-zone box list**: one 52px full-width row per valid target (all players for "one"; a single "EVERYONE AT THE TABLE" row for "all"; only your row for "self") — deck dot, name (Archivo 800 14px), YOU tag, HP right (Archivo 900 15px); `#16121F` + hairline at rest; hovered-while-dragging: `#2E2742`, 2px azure border, scale 1.03. Below, anchored bottom-centre, a **large readable preview** of the staged card (206px, 2px ink border, title 18px, 2-line body, effect line). Tap outside cancels. Drag hit-testing: rows start 36px from pane top, 60px stride, 52px hit-band.
- **Arm flash** (after a target is chosen): the card renders large (224px) at felt centre over a caption "MAJA → VIKTOR" (mono 10px), with **curved dotted arrows** (quadratic bezier bowing toward felt centre; `stroke-dasharray:1 9`, round caps, 2.5px, deck ink, arrowhead marker, slow dash-flow animation, group opacity 0.5 default) to the target — or an **AOE wave** for everyone-cards: two expanding felt-shaped ellipse strokes in deck ink (scale .12→1, opacity .8→0, 2s ease-out, staggered 1s). After ~800ms the card flies (450ms, ease `cubic-bezier(.2,.8,.3,1)`) into its slot in the ARMED stack, scaling to 0.26 and fading to 0.3.
- **ARMED stack** (left felt edge, vertically centered, 58px wide): "ARMED n" header (8.5px violet on a dark pill), one mini per play (58×46, `#2E2742`, 1.5px ink border, "2 → VIKTOR" line + 2-line title, 240ms pop-in), "TAP TO EDIT" footer (mono 7.5px). Each mini keeps a **persistent curved dotted arrow** (2px, ink, `1 9` dashes, slow flow, ~35% opacity) to its target; AOE plays show the wave instead. **Tap a mini to take the card back**: returns to hand, pulls refunded, logged "MAJA TAKES BACK …".
- **LOCK IN · n QUEUED** resolves every armed play at once: log gains "— REVEAL —" + "MAJA LOCKS IN" + one line per effect ("MAJA HITS EMIL −5", "MAJA +4 HP", "MAJA DRAINS NORA −3 PULLS", "MAJA SHIELDS EVERYONE 2"), HP updates, chips flash/shake, stack clears.

### MULLIGAN overlay (once per game)
Full-screen, rgba(11,9,16,.94) + 3px blur, 180ms fade. Kicker "MULLIGAN — ONCE PER GAME" (9.5px violet), title "SWAP YOUR HAND" (Archivo 900 28px), sub "Pick as many cards as you like — even all of them. Replacements come off your deck." Counter "n CHOSEN" (mono 11px, nowrap, right-aligned, clear of the grid). 2-column grid of compact card faces; selected: `#2E2742`, 2px violet border, -5px lift, violet order badge (22px circle, Archivo 900 12px, top-right overhang). Footer: CANCEL secondary + "SWAP n CARDS" (violet fill when ≥1 picked, `#251F35`/`#6A6480` disabled). Confirm replaces picks with draws, logs "MAJA SWAPS n".

### LOG tab
Newest first, 13px Space Grotesk .04em `#A79FBB`; hit/eliminated lines `#F2EEF8`; round markers ("— ROUND 3 —", "— REVEAL —") 11px .14em `#6A6480` with 8px top margin. Copy follows `lc_log()` vocabulary exactly.

## Interactions & Behavior
- **Two paths to play a card**, both ending in the same targeting overlay: (1) hand wheel → tap card → inspect → "PLAY ON THE TABLE →"; (2) table tray → tap (stage) or press-drag ≥10px (a ghost mini follows the pointer: scale 1.15, -4° tilt, 2px ink border, no pointer events).
- Pointer events with `setPointerCapture` (mouse + touch); `touch-action:none` on tray minis and wheel stage. **The drag ghost, drop hit-testing and overlay must all key off the same pane-level geometry.**
- Arming is confirm-free; editing = take-back from the ARMED stack; LOCK IN is the only commit.
- Motion: 130–280ms, always `cubic-bezier(.2,.8,.3,1)`; dash-flow 1.4–2.4s linear; wave 2s ease-out; honor `prefers-reduced-motion` (the live sheet already does).

## State Management
Per viewer: `tab`, `wheelFocus` (float during drag, snapped int at rest), `staged` (card id), `drag` {id, x, y, over}, `pending` (the arm flash) — then per game: armed queue [{card, target}], pulls, hand, mulliganUsed, locked, log. In production the queue is the engine's existing armed-plays list; take-back maps to an unarm route; LOCK IN is the existing lock-in; resolution is the reveal beat.

## Design Tokens (verbatim from lastcall.css)
- Grounds: page `#0B0910`, device `#0E0C14`, panel `#16121F`, panel-alt `#17141F`, raised `#251F35`, focused `#2E2742`, back `#1B1628`, rail `#2A2340`.
- Text: text `#F2EEF8`, body `#CDC6DD`, secondary `#A79FBB`, label `#8D87A0`, faint `#6A6480`.
- Accents: mint `#4FD6A8`, amber `#FFB570`, rose `#F7768E`, azure `#6FB6FF`, violet `#B48EF7`.
- Hairlines: rgba(242,238,248,.10) / .22 strong.
- Deck ramps (fill / ink / on-fill): beer `#FFB570`/`#FFB570`/`#14101D`; cider `#B48EF7`/`#B48EF7`/`#14101D`; wine `#8B2F4A`/`#D4657F`/`#F2EEF8`; liquor `#F7768E`/`#F7768E`/`#14101D`; soft `#6FB6FF`/`#6FB6FF`/`#0D1620`. Renderers emit deck class names, never hex.
- Shadows: lift-sm `0 3px 0 rgba(5,3,10,.5), 0 8px 16px rgba(5,3,10,.42)`; lift-lg `0 6px 0 rgba(5,3,10,.6), 0 22px 40px rgba(5,3,10,.55)`.
- Type: Archivo 800/900 display (-.03em titles), Space Grotesk 400–700 UI, IBM Plex Mono / ui-monospace for machine text. Self-hosted woff2 already in `drinkinggame/assets/fonts/`.
- Ease: `cubic-bezier(.2,.8,.3,1)`.

## Assets
No new assets. Fonts are the repo's own woff2 files; all card copy, tab quests and log vocabulary come verbatim from `src/lc_cards.rs`, `src/lc_tabs.rs` and `src/lc_render.rs`.

## Files
- `Last Call Mobile.dc.html` — the interactive prototype (single file: template + logic). Open in a browser; the design renders as a 390×844 phone frame.
- Repo source this design was built against: `drinkinggame/assets/lastcall.css`, `assets/lc_wheel.js`, `src/lc_render.rs`, `src/lc_cards.rs`, `src/lc_tabs.rs`, `templates/lc_room.html` (branch `master`).

Note: the prototype references a `_ds/...` design-system bundle from its authoring environment; those references are cosmetic scaffolding — every style needed for implementation is inline in the file and documented above.
