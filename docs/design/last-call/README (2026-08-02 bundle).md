# Handoff: Last Call — a card-battler game mode for `/drinks`

## Overview

**Last Call** is a third game mode for the Drinks app (the separate crate at `/drinks`,
alongside Ring of Fire and 3 Man). 2–8 players, phones as private hands, optional TV as a
spectator "big screen". Each player picks a deck matched to what they are actually
drinking; the drink in front of them is the resource the game runs on. Last player above
0 HP wins.

Structurally it is a third value of `games.kind` in the existing schema — same room, same
login, same SSE hub, same `personalize()` hidden-information contract. The one genuinely
new server-side thing is a phase clock, which is the job the already-spawned 1 Hz ticker
in `mechanics.rs` has been waiting for.

## About the design files

The `.dc.html` files in this bundle are **design references created in HTML** —
prototypes showing intended look, layout and behaviour. They are **not production code to
copy**. They render a design-tool runtime (`support.js`) and are laid out as a flat
review canvas, not as an app.

The task is to **recreate these designs inside the existing `drinkinggame` crate** using
its established patterns: Askama templates, HTMX with `hx-boost`, the SSE broadcast hub,
`swapPanel()`, `personalize()`, and the existing `game.css`. Do not introduce a
framework, a build step, or a client-side state store. The repo is
`Hampternt/drawingportfolio`, branch `master`, subtree `drinkinggame/` — see `github.md`
in the project root.

## Fidelity

**High fidelity.** Colours, type, spacing, card anatomy and screen composition in
`Last Call - Game UI.dc.html` are final and should be matched closely. The phone frames
are drawn at exactly 390×844 and the spectator screens at exactly 1920×1080; both are
real target sizes, not illustrations.

The **rules** are draft 0.2 and still unfinished in places — see "Unresolved" at the end of
this README. Build the shell and the loop; expect card content and damage numbers to
change.

---

## Canonical rules

### The vessel economy

The core mechanic, and the thing to protect through every later change:

- Drink is measured in **pulls**. A vessel ≈ one standard drink, split into a
  deck-specific number of pulls.
- Cards cost pulls. Playing cards drains your vessel.
- **Drawing is not automatic and not free.** In the draw beat you may discard anything you
  don't want; the only way to *gain* cards is to **finish your vessel, open a fresh one,
  and take a full hand on top of everything you kept.**
- Hold — drink nothing, draw nothing — is legal and gets quieter every round. The event
  deck is what punishes it.
- **Opening mulligan:** first round only, throw the whole hand back and redraw as many
  times as you like. No cost, no drink.

### Decks

Five decks. Vessel size sets hand size; character comes from card cost and violence.

| Deck | ABV band | Standard vessel · full level | Plays like | Dot colour |
|---|---|---|---|---|
| Beer | 0.5–7% | 50cl can · **8 pulls** | Attrition. Cheap, small, endless. | `#ffb570` |
| Cider | 0.5–7% | 50cl bottle · **10 pulls** | Trickster. Redirects, swaps, theft. | `#b48ef7` |
| Liquor | 20%+ | 4cl measure · **4 pulls** | Burst. Few cards, all of them mean. | `#f7768e` |
| Wine | 7–20% | 15cl glass · **6 pulls** | Control. Curses, delays, damage over time. | `#8b2f4a` |
| Soft | 0–0.5% | Any cup · **6 pulls** | Support. Shields, heals, sabotage. Zero alcohol. | `#6fb6ff` |

Pull count is a **deck constant** — a beer vessel is 8 pulls whether the can is 50cl or
33cl; the app only tells you what fraction of the tin a pull is. The container is
declared once at seating and the app owns all the arithmetic; it never asks a player to
divide anything.

Cocktails and jugs are *several* vessels, not one big one. Deck switching is allowed at a
round boundary only, and requires finishing the current vessel first. Anything under 0.5%
is Soft.

### Life and elimination

- **20 HP** for everyone regardless of deck.
- **0 HP = out.** You finish what's left in your vessel and become a **ghost**.
- **Ghosts** get one vote per round to add +1 damage to an attack already in flight.
- Simultaneous kills are a shared win.

### The round — six beats

Beats advance the moment everyone is ready. The clock is a backstop against the person
rolling a cigarette, not a pace-setter. A round runs 3–5 minutes. Only beat 4 is hidden.

| # | Beat | What happens | Accent |
|---|---|---|---|
| 1 | **Draw** | Discard freely. Optionally finish your vessel to draw a fresh hand on top of what you kept. | `rgba(242,238,248,.32)` |
| 2 | **Deal** | The round's event flips face up for the table. Quests are dealt privately, to some players and not others. | `#ffb570` |
| 3 | **Diplomacy** | Open floor. Deals, threats, lies, voluntary card reveals. Ends when everyone taps Ready. Soft 3-minute cap. | `#b48ef7` |
| 4 | **Lock** | Cards and targets committed in secret. Everything flips at once when the last player locks. | `#b48ef7` |
| 5 | **Respond** | Blocks, counters, traps. Responses resolve **before** the thing they answer. | `#d99a48` |
| 6 | **Resolve** | Remaining effects fire in play order. Damage lands, quests check, drink instructions go out. | `#ff6b5e` |

**Turn order is set by cards played, most first.** Ties broken by a die roll that is shown
on screen. This means the player who acts first is reliably the player who has drunk the
most this round.

**Response rules:** one layer only — responses cannot be responded to. No stack. Response
cards are ordinary cards flagged response-only, so you must already be holding one. You
may respond on someone else's behalf.

**Resolve rules:** effects fire top to bottom with one narrated line each. Targets lock at
reveal — if your target dies before your card resolves, it fizzles. Quests check at end of
resolve; completion is announced, contents are not. **One drink instruction per player at
the end of the round, never mid-beat.**

### Handicaps

A per-player cost multiplier, set at seating, visible to everyone. Framed as a
competitive handicap, not a health warning.

- **×0.5 Lightweight** — cards cost half the pulls, rounded up. Same hand, same damage.
- **×1 Standard** — printed cost, the default.
- **×2 Heavyweight** — double cost. Self-imposed, or the table's punishment for last
  game's winner.
- **Soft deck** is not a multiplier, it's a deck. The honest way to play sober.

Explicitly **not** building: forced water rounds, slow-down nudges, cutoff warnings.

### The party layer — events, quests, pacts

All three ship **off by default**, toggled at room setup. A first game should be legible.

**Events** — public, loud, random. One dealt face up at beat 2, before diplomacy; lasts
exactly one round; never two at once. Recommendation carried into the UI: **deal one and
telegraph the next**, so diplomacy can trade on a future both parties can see.

| Event | Effect |
|---|---|
| Happy Hour | All cards cost half, rounded up, for one round. |
| Lights Up | Hands are public for the round. |
| Last Orders | Play at least one card or take 2. |
| Lock-In | No response beat. Whatever is revealed, lands. |
| Toast | Everyone drinks 1 pull together; everyone gets +2 pulls. |
| Two for One | First player to finish a vessel this round deals +2 with everything. |
| Round of Shots | Any two players may swap one card face down, if both agree. |
| Double Vision | Every attack hits the player seated left of its target. |
| Bar Tab | A random player carries a 3 HP bounty, paid to whoever lands the most damage. |

*Guardrail:* an event may never change how much someone has to drink by more than one
pull, and never targets a single named player except through Bar Tab.

**Quests / tabs** — private, quiet, chosen. One dealt at seating; you draw a new one when
you settle the old. **Not everyone has one** — the uneven deal is the feature. They pay in
**HP and pulls, never in winning.** Settling is announced publicly ("Sara settled a tab")
without saying what it was. Unsettled tabs are revealed at end of game. Three families:
**Vendettas** (aimed at one person, who doesn't know), **Habits** (a play style you must
maintain), **Favours** (someone else has to do well).

**Pacts** — build last, and only if the group enjoys being lied to. At seating the app
quietly pairs two players with matching tabs: *"if you and your partner are the last two
standing, you both win."* Neither is told who the other is. One free one-word "wink" per
pact per game. Only ever one pact, only at 5+ players.

---

## Screens

Last Call slots into the existing three-tab room shell. The **GAME** tab becomes the
board; **ROOM** relabels to **TABLE** while a game runs (the pattern 3 Man already uses);
STANDINGS is untouched. The big screen reuses `/room/{code}/screen`, the same QR join box
and the same SSE stream — no second connection, no second auth. Everything on the TV also
exists on the phone, just smaller; the TV is never required.

### Phone — 390×844, the GAME tab

Vertical stack, top to bottom. Frame `#0e0c14`, 1px `rgba(242,238,248,.16)` border, 26px
radius (device frame only — the real page is edge to edge).

1. **Status strip** — 40px tall, 0 20px padding. Clock left, `ROOM 4KQ2` right. IBM Plex
   Mono 500 12px, `#8d87a0`.
2. **Phase banner** — 0 14px padding. Beat name in Archivo 900 26px/1, `#f2eef8`,
   `-.02em`, uppercase; right-aligned meta in Space Grotesk 600 10px `.12em` uppercase
   `#8d87a0` ("Round 6 · beat 1 of 6", or "2 of 4 ready"). Below it a **six-segment beat
   bar**: 4px tall, 2px radius, 4px gap, one flex segment per beat — past beats at 40%
   accent, current at full accent, future at `rgba(242,238,248,.13)`.
3. **Opponent rail** — horizontal row of equal cards, 8px gap, 0 14px padding. Each card
   `#17141f`, 1px `rgba(242,238,248,.12)`, 8px radius, 7px 8px padding: name (Archivo 800
   12px) + HP (Archivo 800 13px, `#ff6b5e` when low, else `#f2eef8`); a **vessel strip**
   of one 4px segment per pull in the deck's dot colour, empty segments
   `rgba(242,238,248,.13)`; a deck dot (6px circle) plus card count in Plex Mono 500 9px.
   Tap an opponent to target them.
4. **Content area** — `flex: 1`, padding `0 30px 0 14px` (right padding clears the tab
   handles). This is the part that changes per beat.
5. **Action bar** — `padding: 12px 14px 18px`, 1px top border, background `#0e0c14`.
   Primary actions 56px tall, 8px radius. Below them the **HAND / TABLE segmented
   switch**: `#17141f` track, 3px padding, two 38px halves, active half `#b48ef7` with
   `#14101d` label, Archivo 800 12px `.1em`.
6. **Side tab handles** — absolutely positioned at `right: 0`, vertical writing mode, 7px
   radius on the left corners only, 13px 6px padding, Archivo 800 10px `.14em`. `EVENT`
   is amber `#ffb570` on `#1a1206`; `QUEST` is `#262232` on `#cdc6dd`. They pull open
   sheets that cover the content area but **never** the phase banner or the action bar.

#### Cards

A grid, never a fan — a fan dies past six cards. **3 columns up to 9 cards, 4 columns
beyond that; 16 is the practical ceiling.** 8px gap, each card 132px tall (3-col),
`padding: 9px 8px`, flex column.

- Face `#2e2742`, border `1px solid rgba(242,238,248,.3)`, radius 8px.
- Stacked-card shadow: `0 3px 0 rgba(5,3,10,.5), 0 8px 16px rgba(5,3,10,.42), inset 0 1px
  0 rgba(242,238,248,.09)`.
- **Cost chip** top-left: Archivo 800 11px, 3px radius, `1px 6px` padding. Cost 1 =
  `#4a4363` on `#f7f3ea`; cost 2 = `#d99a48` on `#191624`; cost 3 = `#c4382c` on
  `#f7f3ea`.
- **Type tag** top-right: Space Grotesk 700 8px `.1em` — `ATK`, `RESP`, `TRAP`, `HEAL`.
  `#8d87a0` normally; a response card is `#4fd6a8` and its card border becomes
  `rgba(79,214,168,.45)`.
- **Name** Archivo 800 13px/1.15 `#f2eef8`, 8px below the header row.
- **Rules text** Space Grotesk 400 10px/1.3 `#a79fbb`, pinned to the bottom with
  `margin-top: auto`.
- **Marked for discard:** whole card to `opacity: .42`, name gets `line-through`, type tag
  becomes `DISCARDING` in `#ff6b5e`.

Real card copy used in the mocks: *Chip Shot* (2, ATK, "4 damage to one player"),
*Bottle Flick* (1, ATK, "2 damage to one player"), *Coaster* (1, RESP, "Absorb 3 ·
response only"), *Sticky Floor* (2, TRAP, "Cancel the first card aimed at you"),
*Backwash* (1, "Swap a card with the player left of you"), *Tab Slam*, *Neat Pour*,
*Designated Driver*. These are placeholders for balance purposes but the *voice* is
right: bar objects, two clauses maximum, no flavour text.

#### Vessel card

Sits directly above the action bar so the decision and its price are in one glance.
`#17141f`, `1px solid rgba(255,181,112,.4)`, 12px radius, `13px 14px` padding. Header row:
"Your vessel · beer" in Space Grotesk 700 10px `.12em` uppercase `#ffb570`, and
"2 / 8 pulls left" in Plex Mono 500 10px `#8d87a0`. Below: one 26px-tall, 4px-radius
segment per pull, 3px gap — full pulls `#ffb570`, spent `rgba(242,238,248,.11)`. Then one
line of Space Grotesk 400 11.5px/1.4 `#a79fbb` explaining the current option.

#### Beat 1 · Draw

The only beat with two competing primary actions, so they share a row and the drinking one
is amber:

- **FINISH & DRAW** — flex 1, 56px, `#ffb570` on `#1a1206`. Archivo 900 17px label with a
  Space Grotesk 600 9.5px `.1em` sublabel at 72% opacity: `EMPTY THE CAN · +5 CARDS`.
- **KEEP** — 104px wide, 56px, transparent with `1px solid rgba(242,238,248,.22)`,
  `#cdc6dd`. Archivo 900 15px + `NO DRAW` sublabel in `#8d87a0`.

Hand header row: "Your hand · 5 cards" (Space Grotesk 700 10px `.13em` uppercase
`#8d87a0`) left, "Tap to discard" (same type, `#b48ef7`) right.

#### Beat 3 · Diplomacy, event sheet open

Content area gets a scrim: `position: absolute; inset: 0; background: rgba(11,9,16,.72);
backdrop-filter: blur(3px)`.

- **Event sheet** — `left: 14px; right: 30px; top: 10px`, `#17141f`, `1px solid
  rgba(255,181,112,.45)`, 14px radius, `18px 18px 20px`, shadow `0 18px 44px
  rgba(0,0,0,.55)`. Kicker "Event · this round only" `#ffb570` + deck counter
  `EVENTS 11 · DISC 4` in Plex Mono. Title Archivo 900 32px/1.02 `#ffb570` uppercase.
  Body Space Grotesk 400 14px/1.45 `#ece8f5`. Two metadata tiles side by side —
  **AFFECTS** / **EXPIRES** — on `rgba(242,238,248,.05)`, 8px radius. Then a divider and
  **"Next round, already visible"**: a 34×44 dashed placeholder card beside the next
  event's name and one-line effect in `#8d87a0`.
- **Ready check** — pinned `bottom: 12px`, same left/right insets. One row per player: 8px
  status dot (`#4fd6a8` ready, `rgba(242,238,248,.22)` not), name in Space Grotesk 700
  13px, state in Plex Mono 500 10px (`READY` / `TALKING` / `—`).

#### Remaining phone beats

`Last Call - Game UI.dc.html` also draws **Lock** (arm a card, aim at an opponent, LOCK IN),
**Respond** (PLAY COASTER against a specific incoming attack), **Resolve** (the ordered
play list with HP deltas like `12 → 9`), a **Table** view for rooms with no TV (a third tab
beside HAND and LOG), and a **card-art creator** flow (New card → Fit to card → Use this
crop → Save to Beer deck → Enable in room). Read the file for these; the vocabulary above
is consistent across all of them.

Card art is **5:7, resized on upload to 640×896 and stored as WebP** — one aspect for the
phone grid, the enlarged view and the big screen.

### Big screen — 1920×1080

Two variants are drawn: a spectator layout and a "the table" layout. Both are three-column
with a bottom strip, background `#0b0910`, 1px hairline column dividers
`rgba(242,238,248,.1–.12)`, 24–30px column padding.

- **Left column** — `Event · this round` (amber-bordered `#17141f` card, title Archivo 900
  38px `#ffb570`), `Next round` (dashed-border card, title Archivo 900 24px `#cdc6dd`),
  and at the bottom either `Quests in play` or `Seat order`.
- **Centre** — the arena. Players as HP columns around the edge; at reveal, attacks drawn
  as arcs across the middle. Below it, **"On the table — resolves top to bottom"**: one
  `#1d1929` row per play, 12px radius, coloured border per outcome (`rgba(79,214,168,.5)`
  for a successful response).
- **Right column** — `Feed`, a kill feed in IBM Plex Mono 500 15px behind a `>` prompt,
  colour-coded (`#4fd6a8` for your own actions), and a `Tonight` stats grid.
- **Bottom strip** — 132px tall, `Decks left / disc`: six tiles showing each deck's
  remaining and discard counts with its colour dot.

**Legibility floor:** nothing below 11px at 1080p. HP at 38px, the phase name at 56px, the
timer in mono so digits don't jitter.

---

## Interactions & behaviour

- **Targeting:** tap a card to arm it, tap an opponent in the rail to aim, tap LOCK to
  commit. Locked state is visible to the table as a tick in the rail and a lock light on
  the big screen.
- **Un-ready:** during diplomacy anyone may un-ready until the last player taps in.
- **Reveal:** everything flips at once. This is the screenshot moment — the existing
  `swapPanel()` anim-key flip already handles it.
- **Narration:** resolve emits one line per effect, in order, to both the phone log and
  the big-screen feed.
- **Deck exhaustion:** every deck shows a remaining count and a discard count. Under five
  cards the counter turns amber; at zero the discard reshuffles.
- **Motion:** 130ms controls, 190ms surfaces, 280ms overlays, all
  `cubic-bezier(.2,.8,.3,1)`. Press is `translateY(1px)`, never a scale. No bounce, no
  parallax. `prefers-reduced-motion` zeroes every duration.
- **Focus:** the site-wide double ring — 2px page colour then 2px violet — on every
  control.

## State

Whole match state serialises into `games.state_json`, exactly like `three_man.rs`. No new
tables needed for v1.

Per match: `round`, `beat` (1–6), `beat_deadline`, `event_current`, `event_next`,
`turn_order` (derived from cards played, ties by a shown roll), `pact` (optional pair).

Per player: `deck`, `vessel_pulls_max`, `vessel_pulls_left`, `hp`, `handicap`, `hand[]`,
`committed[]` (card + target), `quest`, `ready`, `locked`, `is_ghost`.

Hidden information is handled entirely by the existing `personalize()` attribute contract
— one rendered panel, `data-show-player` reveals each player only their own cards. Every
action route goes through the existing per-room async lock; this matters more here than in
the other modes because eight people lock in at once.

Phase transitions are ordinary `game` / `screen` SSE events. The phase clock is the 1 Hz
ticker in `mechanics.rs`.

## Design tokens

Taken from `drinkinggame/assets/game.css` and the Hampter design system.

**Surfaces** — page `#0b0910` · raised `#0e0c14` · panel `#14111c` · card `#17141f` ·
elevated `#1d1929` · card face `#2e2742` · chip `#262232` · cost-1 chip `#4a4363`

**Text** — primary `#f2eef8` / `#ece8f5` · secondary `#cdc6dd` · muted `#a79fbb` ·
faint `#8d87a0`

**Accents** — violet `#b48ef7` (brand, links, focus, active tab) · amber `#ffb570`
(drinking, events, warmth) · deep amber `#d99a48` (cost-2) · rose `#f7768e` · red
`#ff6b5e` (danger, elimination) · brick `#c4382c` (cost-3) · mint `#4fd6a8` (success,
responses) · azure `#6fb6ff` (soft deck) · wine `#8b2f4a`

**Hairlines** — `rgba(242,238,248,.10)` divider · `.12–.18` border · `.32` heavy rule ·
`rgba(242,238,248,.05–.055)` inset fill

**Type** — Archivo 800/900 for headings, wordmarks, card names, numbers; tracking −.02 to
−.045em on display sizes. Space Grotesk 400/500/600/700 for prose, labels and controls.
IBM Plex Mono 500 for anything machine-produced: clocks, room codes, counts, HP deltas,
the `>` feed prompt. Micro-labels are Space Grotesk 600/700 at 9–11px, uppercase, tracked
.10–.18em. **Sans is never uppercased at prose sizes; mono is never used for paragraphs.**

**Radii** — 3px cost chips · 5px small controls · 8px cards and buttons · 10–12px panels ·
14px sheets · 26px phone frame

**Shadows** — card `0 3px 0 rgba(5,3,10,.5), 0 8px 16px rgba(5,3,10,.42), inset 0 1px 0
rgba(242,238,248,.09)` · sheet `0 18px 44px rgba(0,0,0,.55)` · big-screen panel
`0 12px 40px rgba(5,3,10,.6)`

## Assets

- **Fonts** — `drinkinggame/assets/fonts/*.woff2`, Archivo 500–900 and Space Grotesk
  400–700. These are the files the crate already compiles in; bundled here unchanged.
- **IBM Plex Mono** — not self-hosted in the repo. The design uses it via
  `'IBM Plex Mono', ui-monospace, monospace`. Either self-host it or accept the system
  fallback.
- **Icons** — none. The design uses text labels, coloured dots and bars throughout. No
  icon set is required.
- **Card art** — none exists. Cards are text-only for the first build; the creator flow in
  the UI file is a later phase.
- **Design system** — the Hampter tokens are bundled under `_ds/` for reference. The
  prototypes are inline-styled and do not depend on them at runtime.

---

## Unresolved — read before building

The documents in this bundle agree with each other and with the screens — an earlier
draft's economy (a per-round pull budget spent across cost-racked decks, a separate chug
action, and an end-of-round spill) has been removed from all three. If you find any trace
of a "draw allocator", a "rack", a "budget bar" or a "spill" rule, it is stale and the
screens win.

What remains genuinely undecided, each with a working fallback so none of it blocks a
playtest:

| Open | Fallback if nobody decides |
|---|---|
| No card list exists for any deck | Beer only, ~12 cards, paper first |
| Deck size and fresh-draw size — screens show five cards, nothing justifies five | 24 cards a deck, draw five, discards reshuffle when it runs dry |
| Hand ceiling — nothing caps a hand now that spill is gone | No hard cap; the pull cost of *playing* is the limiter. Watch for hoarders. |
| Damage numbers — what does a 1-cost hit for against 20 HP? | 1-cost = 1 damage, derive the rest from the paper test |
| Targeting rules — stacking, self-target, splitting | One card one target; stacking allowed; self-target only for heals and shields |
| Game length — 20 HP over 3–5 min rounds may be too long | Expect 5–8 rounds; 12–15 HP is the likely correction |
| Which cards are reactions, and how you get them | Normal cards flagged react-only; you must already hold one |
| Ghost powers beyond the +1 vote | Ghosts see everything and count for nothing |
| Handicap fairness — nothing stops everyone picking ×0.5 | Handicaps are set by the table at seating, not by the player |
| Tab supply, repeats, dead-target tabs | A dead target voids the tab and you redraw |
| Disconnects | Two missed commits and you're folded out, HP intact |
| Table minimum — react/ghosts/pacts all assume a crowd | Four minimum; 2–3 is a stripped duel mode or nothing |
| Vessel honesty — nothing verifies a pull happened | Social. The table can see the can. Never build enforcement. |
| Safety stance around chug | Must be decided before anyone outside the group plays it |

**Anti-turtle lever, not yet needed:** if playtesting shows passive play winning, add an
escalating **Last Call clock** — from round 8, everyone takes 1 damage per round. Prefer
that over a hold penalty; it's one rule and it creates an ending.

## Suggested order

1. **Paper playtest.** Index cards, one evening, five people. Answers the damage baseline
   and whether turtling is real, for the price of a marker pen.
2. **Card list, one deck.** Beer only, ~12 cards. If beer alone is fun for four players,
   the other decks are flavour work.
3. **Build the shell.** New `games.kind`, phase clock on the existing ticker, panels
   through the existing SSE hub, `personalize()` for hidden hands.
4. **Party layer.** Events first — pure server logic, needs no UI beyond a banner. Quests
   next. Pacts last, if at all.

## Files in this bundle

| File | What it is |
|---|---|
| `Last Call - Game UI.dc.html` | **The build reference.** Phone beats at 390×844, two 1920×1080 big-screen layouts, card-art creator, no-TV table view. |
| `Last Call - Design Doc.dc.html` | The full rules document, draft 0.1. Sections 10 and 11 are the open-question lists. |
| `Last Call - A Round, Step by Step.dc.html` | One round played out beat by beat with three players, real numbers, and the assumptions it had to invent listed at the end. |
| `Last Call - Pitch.dc.html` | The short pitch. |
| `support.js`, `doc-page.js`, `image-slot.js` | Runtime for the prototypes. Not part of the design. |
| `drinkinggame/assets/fonts/` | Archivo + Space Grotesk woff2, straight from the repo. |
| `_ds/` | Hampter design system tokens, for reference. |

Open the `.dc.html` files directly in a browser.
