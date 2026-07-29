# Ring of Fire — Design

**Date:** 2026-07-29
**Status:** Approved

## Summary

The first actual game for the drinking-game section: a digital 52-card stack
simulating Ring of Fire with the group's modified rules. It runs **inside the
existing drink-tracker rooms** (see `2026-07-28-drinking-game-v1-design.md`):
everyone already in a room sees the game on their phone, the spectator screen
shows it large, and card draws broadcast live over the room's existing SSE
channel.

## Decisions made during brainstorming

- **Play model**: shared digital card stack across all phones in the room —
  whoever taps the deck draws; the card is shown on every screen, including
  the spectator/TV view.
- **Turns**: none. A draw is attributed to the player whose phone tapped.
  Per-player draw totals are displayed.
- **Held cards**: some ranks are "holdable" (default: 5 = Thumb Master,
  7 = Heaven) — the draw stays with its holder until spent. **Multiple
  holders of the same rank coexist**; each spends independently.
- **Game end**: play until the deck is empty, then show a summary.
  Kings are ordinary rule cards (no King's Cup end-trigger).
- **Rules**: saved **presets** stored server-side. Default preset is the
  group's standard: 4 = Whores, 6 = Dicks (rest classic).
- **Tracker tie-in**: none in v1. The existing +1 Drink / +1 Shot buttons
  stay visible and fully manual; card draws log nothing to the tracker.
- **Room model**: no separate lobby — a room gains a "Start Ring of Fire"
  action. One join code per game night.
- **State**: DB-backed (approach A) — shuffled deck order and every draw
  persist in SQLite, matching v1's no-client-state philosophy. A refresh,
  locked phone, or server deploy mid-game recovers correctly.

## Data model

New migration in the drinkinggame crate's own DB (`drinkinggame.db`):

- `rule_presets` — `id`, `name` (unique), `rules_json`, `created_at`.
  `rules_json` is a serde-serialized array of 13 entries:
  `{ rank: 1..=13, title, text, holdable: bool }`.
  The migration seeds a **"Standard"** preset with the group's rules
  (Ace = Waterfall, 2 = You, 3 = Me, 4 = Whores, 5 = Thumb Master,
  6 = Dicks, 7 = Heaven, 8 = Mate, 9 = Rhyme, 10 = Categories,
  J = Make a Rule, Q = Questions, K = King's Cup; 5 and 7 holdable).
- `games` — `id`, `room_id`, `rules_json` (snapshot copied from the preset
  at start — editing a preset never mutates a running game), `deck_order`
  (text: the shuffled 52 cards, e.g. `QS,3H,AC,…`), `created_at`,
  `ended_at` (nullable). A partial unique index on `room_id WHERE ended_at
  IS NULL` enforces one active game per room.
- `game_draws` — `id`, `game_id`, `player_id`, `card_index`, `drawn_at`,
  `spent_at` (nullable tombstone, mirroring the tracker's `undone_at`).
  Card identity = `deck_order[card_index]`.
  `UNIQUE(game_id, card_index)` turns double-tap races into a clean
  constraint conflict instead of a duplicate draw — the loser of the race
  gets the *next* index on retry, handled server-side in one transaction.

Deck storage rationale: persisting the shuffled order (~150 bytes of text)
beats persisting an RNG seed — seed derivation couples correctness to the
RNG algorithm never changing across dependency upgrades.

## Game flow

1. **Start**: the room view gains a Ring of Fire panel with a preset picker
   (default preselected) and a Start button. Starting shuffles a 52-card
   deck (Fisher–Yates via the `rand` crate — new dependency of the
   drinkinggame crate), snapshots the preset's rules into the game row, and
   broadcasts the game panel to all room screens.
2. **Draw**: any member taps the deck → `POST` draws the next undrawn index,
   attributed to the tapper's session player. The broadcast fragment shows:
   large card face, rule title + text, who drew it, cards remaining, and
   per-player draw counts.
3. **Held cards**: a draw whose rule is `holdable` lands in a "held cards"
   strip (card + holder name) on all screens. Only the holder's own phone
   renders a **Use** button → `POST` sets `spent_at` and broadcasts an
   announcement ("X used Thumb Master!"). Multiple holders per rank coexist.
4. **End**: drawing the 52nd card auto-ends the game (`ended_at` set) and
   broadcasts a summary (per-player draw totals). An "End game early"
   button (any member) does the same. The room returns to idle with Start
   available again.

Throughout, the tracker's drink/shot/undo buttons remain visible and manual.

## Presets page

`/drinks/presets` (auth required, any logged-in player — it's a friends app):

- List all presets.
- Create: copies an existing preset (or Standard) as the starting point.
- Edit: form with the 13 fixed rank labels (A, 2–10, J, Q, K), each with
  title, text, and a holdable checkbox. Saves the whole set at once.
- Delete: allowed; running games are unaffected (they hold snapshots).
  The seeded Standard preset is recreated by the migration guard only if
  missing, so deleting it is permitted but it returns on next deploy —
  acceptable quirk for v1.

## Rendering & SSE

- Card faces are pure HTML/CSS — rank text + suit glyph (♠ ♥ ♦ ♣),
  red/black coloring. No image assets; preserves the crate's
  single-binary embed property.
- The game panel is a new fragment kind broadcast over the **existing**
  per-room `tokio::sync::broadcast` channel; HTMX swaps it into a container
  div present on both the room page and the spectator screen. The spectator
  layout shows the current card large.
- On page load/reconnect the room page renders the current game state
  server-side — SSE only pushes deltas; there is no client-side game state.

## Error handling

Typed domain errors (existing `thiserror` pattern) mapped to friendly HTML
fragments: drawing with no active game, drawing an exhausted deck, spending
a card you don't hold (or already spent), starting a game while one is
active, unknown preset.

## Testing

- **Unit** (db layer): shuffle produces exactly 52 unique cards; draws come
  back in deck order; double-draw on the same index conflicts; spend
  semantics (only holder, only once); preset CRUD round-trip incl. JSON
  (de)serialization; auto-end on last card; one-active-game constraint.
- **Integration** (`tower::ServiceExt` against in-memory SQLite): full
  start → draw ×N → spend → end flow; error paths above.
- **Manual**: SSE live updates across two browser windows + spectator view,
  and phone-sized layout, verified in a real browser before calling it done.

## Out of scope for v1

- Auto-logging drinks from card draws (draw history is persisted, so a
  future version can add it without schema changes)
- Jack "make a rule" running text list (verbal at the table)
- Turn enforcement of any kind
- Card animations beyond simple CSS transitions
- Preset permissions/ownership
