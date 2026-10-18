# Drinks — the table view for Ring of Fire and 3 Man

**Status:** merged into `dev` 2026-10-18, after the owner's test. Branch `feat/drinks-table-view`.

## Goal

Ring of Fire and 3 Man should feel like they are played around a real table,
the way Last Call's big screen already does:

- **The big screen shows a felt table with the players seated around its
  edge**, as Last Call does.
- **In 3 Man**, the dice sit next to the player whose turn it is, and that
  player's seat is highlighted.
- **In Ring of Fire**, the face-down cards are spread in a loose ring in the
  middle of the table. A player taps a card on their phone's table to pull
  it.
- **Seat order is quick to fix from a phone**, because people move around:
  pick positions from a dropdown list, or drag players to their seats on
  the table.
- **A screen can be in the room without being a player.** Today the only way
  to the big-screen link is to join as a player, which is an oversight.

## Decisions

- **Seat order is stored per room** (chosen 2026-10-18). It is one order that
  every game reads, it carries over between games, and moving someone lasts
  until they are moved again. It replaces today's join-time order
  (`ORDER BY rp.joined_at`). Not chosen: each game snapshotting join order
  and reordering lasting for that game only.
- **Not being a player gets two fixes** (chosen 2026-10-18):
  1. The landing page gets **Open big screen**, which needs only a room code:
     no name, PIN or cookie.
  2. Someone already in the room can **sit out**. They stay in the room and
     can watch, but they are left out of the seat ring, turns, dealing and
     standings until they sit back in.

  Not chosen: the code-only screen entry on its own.
- **Any signed-in player can reorder, from the phone only** (chosen
  2026-10-18), using either a dropdown list or dragging seats on the phone's
  table. The big screen only displays the order. Not chosen: dragging on
  the big screen. The screen has no login, so anyone holding the room code
  could reorder.
- **In Ring of Fire, the tapped card is only what leaves the ring** (chosen
  2026-10-18). The card drawn is still the next one in `deck_order`, so the
  sequential `card_index`, the double-draw conflict retry and their tests
  stay untouched. Players see no difference. Not chosen: each ring slot
  mapped to a real deck position.
- **Ring of Fire keeps its free-for-all draw (no turn order)**, as today. The
  seat ring still shows who drew last. The owner didn't raise turn order;
  recorded here as the default.
- **Where a newcomer sits** (decided 2026-10-18 after the free-form build,
  replacing "players who join mid-game take the last seat"). Until anyone
  has arranged the room (dropped a token, moved the start marker, or used
  the dropdown or ↑/↓), the ring stays evenly spaced in join order with the
  newcomer last in the count, re-spaced on every join, sit-out and sit-in.
  Once the room is arranged, a newcomer lands mid-gap in the widest gap and
  nobody else moves; sitting back in returns a player to their old angle.
  Anyone can move them afterwards. Not chosen: mid-gap just before the
  start marker (it halves the last gap on every join and piles later
  joiners onto the marker), and a minimum spacing on join (it still crowds
  once the gaps run out).
- **Dragging is free-form, and a token stays where you drop it** (chosen
  2026-10-18, after testing midpoint 1). Each player's token sits on the
  table's rim at the angle where it was last dropped, on both the phone and
  the big screen, so spacing can be uneven, the way real people bunch up.
  Seat order is the clockwise order of those angles around the table's
  centre, and each token shows its current number. Not chosen: snapping
  back into an even ring on release, or free x/y anywhere on the table,
  which would collide with the dice and the card ring.
- **Counting starts at a draggable start marker** (chosen 2026-10-18). A
  small marker on the rim says where seat 1 begins, and any player can drag
  it like a token. Not chosen: always counting from 12 o'clock.

## Guidelines

Read first: the repo `CLAUDE.md` (the "Adding a game to `/drinks`" checklist
and the Backroom SSE warning), `docs/design.md` §Drinking game,
`drinkinggame/src/lc_layout.rs`, and Last Call's felt in `lc_render.rs` /
`lastcall.css` / `lc_screen.html`.

- **Build one table and reuse it.** Generalise `lc_layout.rs` and the Last
  Call felt into a shared seat-ring component, with a centre slot each game
  fills: dice for 3 Man, the card ring for Ring of Fire. Use it at both
  sizes, the 1920×1080 screen and the phone pane, as `lc_layout` already
  does. Last Call must look the same afterwards.
- **More than 8 seats.** `RING` covers only 2–8 (Last Call's `MAX_SEATS`),
  but Ring of Fire and 3 Man rooms have no cap. Generate rows past 8 and
  keep the authored ones for Last Call. The ring must stay legible at 12 or
  more.
- **The live marker stays Last Call's.** Ring of Fire and 3 Man screen panels
  must not carry `data-lc-live`, which
  `test_exactly_one_screen_panel_builder_marks_itself_live` pins. A changed
  screen panel goes through all three dispatch arms in `game.rs`
  (`current_panel`, `current_screen_panel`, `current_room_panel`).
- **Storing seat order** is a drinkinggame migration: a position column on
  room membership, plus a sitting-out flag. drinkinggame uses runtime-checked
  sqlx, so there is no `cargo sqlx prepare` step. Every game that reads
  members (3 Man's start, Last Call's and Backroom's seating) reads the
  room's order and skips anyone sitting out. Only Ring of Fire and 3 Man
  change visually.
- **Reordering 3 Man mid-game.** `ThreeManState.order` is fixed when the
  game starts, so a reorder needs a phase-guarded engine transition. That
  transition keeps the current roller and the current 3 Man pointing at the
  same people, and bumps `seq` once. Sitting out mid-game is a second new
  transition that removes the seat; the engine has no leave handling today.
  If the person leaving is the roller or the 3 Man, the role passes to the
  next seat.
- **Card scatter is deterministic.** Seed the loose ring's positions from the
  game id so the phone, the screen and every reload draw the same table.
  Drawn cards leave the ring. The last drawn card shows face-up in the
  centre, next to the existing rule text.
- **Dice.** Draw them beside the current roller's seat, which is
  highlighted, and give the 3 Man's seat its own badge. This is a strawman,
  since the ask was loose here, so show it to the owner at the midpoint.
- **Reorder UI.** The dropdown list goes in the ROOM tab. Dragging works on
  the phone's table view with touch (pointer events, not HTML5
  drag-and-drop, which phones don't support).
- **Storing the angle.** A per-player rim angle, plus a per-room start-marker
  angle, is a second drinkinggame migration. Backfill it evenly from the
  current seat order, with the same `column_exists` guard. Keep
  `room_players.seat` as the derived order, rewritten in the same
  `BEGIN IMMEDIATE` transaction as every angle change, so every existing
  `ORDER BY seat` reader stays correct. A drop sends only the dragged
  player's new angle, never a whole order, so two phones dragging at once
  both land. The dropdown's "Seat N" places the player mid-gap at that
  slot. 3 Man's ↑/↓ swaps two angles. Newcomer placement is in
  Decisions (even ring until arranged, then the widest gap). Two tokens at the same angle
  are ordered by player id. Nudge a drop so tokens don't sit exactly on top
  of each other.
- **Where the angle is used.** Ring of Fire, 3 Man, the idle screen and the
  phone's table read the stored angles. Last Call keeps its authored
  `RING` positions; it gets the order only.
- **SSE.** A reorder or sit-out publishes to the room like any visible
  change. Nothing secret is involved, since seat order is public. The
  Backroom rules on what may ride the stream still apply.
- **JS lifecycle.** Any new script follows the hx-boost rules in `CLAUDE.md`
  and gets `?v=` cache busting.
- **Test data:** made-up player names only, through `DRINKS_TEST_MODE=1`
  locally (spawn fake players, hop identities).

## Done when

- The landing page opens a room's big screen from just the code, without
  joining.
- A player can sit out and sit back in. While sitting out, they are on no
  table, in no turn and in no standings.
- In any room, a player can reorder seats from the ROOM tab list, or drag
  any token freely round the phone's table, where it stays where it lands.
  Each token shows its number, counted clockwise from the start marker,
  which can be dragged too. The phone and the big screen update live, and
  the layout is still there for the next game.
- The 3 Man big screen is a table: players around the edge, dice at the
  roller's highlighted seat, the 3 Man badged. Reordering mid-game keeps
  the right person rolling.
- The Ring of Fire big screen and phone show the face-down cards in a loose
  ring. Tapping one on the phone draws, that card leaves the ring
  everywhere, and the drawn card and its rule appear in the centre.
- Last Call looks exactly as before.

## Test it yourself

Start it locally with test mode on:

```bash
DRINKS_TEST_MODE=1 cargo run -p drinkinggame
```

Open `http://localhost:3001`, create a room, and spawn about 6 fake players
from the test panel.

**Midpoint 1: screen entry, sitting out, seat order.**

1. In a second window, use **Open big screen** on the landing page with just
   the code. The screen loads, and nobody new joins the room.
2. As one fake player, sit out. They drop off the seat list everywhere. Sit
   back in.
3. Reorder from the ROOM tab dropdown. Then drag a token part-way round
   the phone's table and let go. It stays there, and the numbers update
   clockwise. Drag the start marker and watch the numbering follow it. The
   big screen follows each change.

**Midpoint 2: 3 Man.**

4. Start 3 Man. The screen shows the table with dice at the roller's seat.
   Roll a few times and watch the dice move.
5. Reorder mid-game. The same person is still rolling, and the 3 Man badge
   didn't jump.

**End: Ring of Fire.**

6. Start Ring of Fire. Cards lie in a loose ring, and the layout survives a
   reload.
7. Tap a card on a phone. It leaves the ring on both phone and screen, and
   the drawn card and its rule show in the centre.
8. Try with 12 or more fake players. Seats stay readable.
9. Start Last Call and check that it looks as it did.

## Built

Built 2026-10-18 on `feat/drinks-table-view`, from 9658dba to the Built
commit. Only the `drinkinggame` crate and the docs changed.

### What changed

**Seat order and sitting out (storage).**
- **Migration 004:** `room_players.seat` (backfilled from join order) and
  `room_players.sitting_out`.
- **Migration 005:** `room_players.angle`, `rooms.start_angle`,
  `rooms.arranged` and `game_draws.ring_slot`.
- Both are `column_exists`-guarded ALTERs in `db::run_migrations`, and both
  backfills only fill NULLs.
- `seat` is now the order derived from the angles, clockwise from the start
  marker. Every seat write goes through `db::rearrange`, one
  `BEGIN IMMEDIATE` transaction that rewrites the angles, `seat` and the
  arranged flag together. That covers drops, marker moves, the dropdown,
  joins, sit-out and sit-in.
- `room_members` returns everyone in seat order, because name lookups
  still need sat-out players. `seated_members` is the ring that 3 Man,
  Last Call (start and rematch) and Backroom seat from.
- The standings leave out anyone sitting out.

**Routes (`seats.rs`, `routes.rs`).**
- `GET /screen?room=CODE` comes from a new "Open big screen" form on the
  landing page. It works signed in or not and joins nobody.
- `POST /room/{code}/seats/move` takes `player_id` and a 1-based
  `position`, and backs the dropdown. The player lands mid-gap at that
  slot.
- `POST /room/{code}/seats/angle` takes `player_id` and `angle`, and is a
  token drop. `POST /room/{code}/seats/start` takes `angle`, and is a
  marker drop.
- `POST /room/{code}/sit-out` and `/sit-in` act on yourself only.
- All of these hold the room lock. Each change broadcasts the ROOM tab, the
  standings, and the game frame. With no game running it sends the screen
  frame instead, so a game-over summary on the phones survives.
- A sat-out player can't draw in Ring of Fire, and a reload doesn't seat
  them in a running 3 Man or Last Call game.
- `room_page`'s and `test_spawn`'s joins now run under the room lock.

**3 Man engine.** Two new transitions, both followed through
`seats::sync_running_game`:
- `reorder`: any phase. The roller and the 3 Man stay the same people, and
  `seq` goes up once.
- `remove_player`: a leaving roller or 3 Man passes the role to the next
  seat. It is refused mid-double, and refused if it would leave fewer than
  two players.

A mid-game join now sits where the room ring puts the newcomer.

**The table (`seat_ring.rs`).**
- Geometry:
  - Last Call's authored 2–8-seat rows moved here, and `lc_layout`
    re-exports them clamped.
  - Every other table puts each token at its stored rim angle, with its
    seat number.
  - The start marker sits on the felt's edge.
- `table_html` takes a centre slot, an overlay for game pieces, a scale
  (phone or screen) and optional drag URLs. It never emits `data-lc-live`.
- `assets/seat_drag.js` drags tokens and the marker along the rim with
  pointer events. A room frame that arrives mid-drag is parked until the
  drag ends.

**ROOM / TABLE tab.**
- WHO'S HERE became THE TABLE: the draggable felt, the same ring as rows
  with a "Seat N" dropdown, SIT OUT on your own row, and a SITTING OUT list
  with SIT BACK IN.
- During 3 Man the felt carries the roller highlight, the 3 MAN badge and
  the dice. Each row gets a 3 MAN button, and the holder's row shows a
  badge.

**Idle big screen.** The ring on the felt, display only.

**3 Man.**
- The big screen is the felt: tokens at their angles, the roller lit, the
  3 Man badged.
- The dice sit inside the felt on the roller's side. They show the roll
  during the turn and go blank once it passes, so they move with the
  roller.
- The sum or WAITING ON line is in the centre; the calls and the
  hand-off / double panels sit beneath the table.

**Ring of Fire (`card_ring.rs`, `assets/card_ring.js`).**
- The big screen and the phone GAME tab show the felt with the face-down
  cards in a loose ring. The last drawn card is face up in the centre, the
  rule text stays beside the table, and whoever drew last is lit with a
  DREW badge.
- The scatter is a fixed SplitMix64 seeded from the game id.
- A tap posts the draw with `slot=N`, stored in `game_draws.ring_slot`
  after `insert_draw`.

### Full gate

`./scripts/verify.sh`, exit 0:

```
=== cargo fmt --check
=== cargo clippy
=== cargo test
test result: ok. 341 passed; 0 failed
test result: ok. 8 passed; 0 failed
test result: ok. 559 passed; 0 failed
test result: ok. 0 passed; 0 failed
test result: ok. 252 passed; 0 failed
test result: ok. 0 passed; 0 failed
=== node --check (static JS)
=== node (the board suites)
ok   docs/design/sorting-live/src/model.test.js — passed 265/265 checks
ok   docs/design/sorting-live/src/board.test.js — passed 193/193 checks
VERIFY OK — fmt, clippy, tests, JS syntax, board suites all clean.
```

That is 1160 workspace tests plus the board's 458 checks. Clippy shows no
`drinkinggame` warnings.

### Test data

Nothing to seed. Run `DRINKS_TEST_MODE=1 cargo run -p drinkinggame`, log
in with any made-up name, start a night, and press **+ FAKE** on the TEST
bar. That spawns `test-1`, `test-2` and so on, all with PIN 0000. The TEST
bar still lists sat-out fakes, which is how you switch into one to sit it
back in.

### Done differently from the plan, and why

- **Seat geometry: angles instead of generated rows.** Seats past 8 were
  first generated rows on Last Call's ellipse. The free-form decision
  replaced them with stored angles. Last Call shares only the authored rows
  and keeps its own `#lc-felt` markup and CSS: its flight anchors, ids and
  tests key on them. The shared felt restates Last Call's colour values in
  `game.css`, because the room and screen pages load only that sheet.
- **Newcomers.** In an unarranged room every join, sit-out and sit-in
  re-spaces the ring evenly in join order, at (i+0.5)·360/n from the
  marker. Once anyone has arranged the room, a newcomer lands mid-gap in
  the widest gap; that gap can span the marker, so the newcomer may become
  seat 1. A room with no seats yet seats its first player opposite the
  marker.
- **Drops.** A drop is nudged 4° off any token it lands on, and never
  across the marker. Sitting back in an arranged room returns the player
  to their old angle, nudged.
- **Marker label.** It reads "SEAT 1", not "START", because the idle panel's
  test pins `>START<` to exactly four game cards.
- **3 Man engine work came forward to midpoint 1** (`reorder` and
  `remove_player`). Without it, sitting out or reordering during 3 Man
  would have left two orders disagreeing. Moving the marker mid-game
  rotates the game's order: neighbours, the roller and the 3 Man stay the
  same, but `seq` goes up.
- **3 Man's ↑/↓ SEATING ORDER list, `/tm/seat`, the swap helpers and the
  screen's bottom seat strip are gone.** THE TABLE, the dropdown and drag
  replace them, and the 3 MAN reassign buttons moved onto THE TABLE's rows.
  The phone GAME tab keeps its small seat strip because the "a 7 is yours"
  line reads it.
- **Last Call and Backroom take the ring when they start.** A seat change
  or sit-out during one applies from the next game, and the ROOM tab says
  so. Taking a seat out of a running Backroom game would break the engine
  and reveal roles.
- **Sat-out players.**
  - Starting 3 Man while sat out hands seat 1 the first roll and the title.
  - A sat-out player can't draw in Ring of Fire (409), but can still tap
    +1 DRINK; those drinks stay hidden from the standings until they sit
    back in.
- **Big screen refreshes.** A rename doesn't refresh the idle screen's
  ring: that path avoids overwriting a game-over summary on purpose. Joins
  and seat changes do repaint the idle screen, so after a game they replace
  the summary on the screen; phones are left alone.
- **Ring of Fire draw path.**
  - A tap picks the card nearest the finger. 52 overlapping cards are too
    small to aim at one by one on a phone.
  - A tapped slot that is already gone, or no slot at all, falls back to the
    lowest card still on the table, so the ring always holds 52 minus the
    number of draws.
  - The TAP TO DRAW button stays as a fallback and sends the same request
    with no slot.
  - Recording the slot is a separate UPDATE after `insert_draw`, so drawing,
    `card_index` and the conflict retry are untouched.
- **No `?v=` on the new scripts.** The drinks crate has no cache-busting
  mechanism and serves its JS without long caching, like the `lc_*.js`
  files.
- **Smaller changes.** New integration tests live at the end of
  `tests/http.rs`. Two 3 Man tests changed: one lost its ↑/↓ steps, and one
  now moves seats through `/seats/move` and checks that `/tm/seat` returns
  404. Smaller test assertions changed to fit the new markup: WHO'S HERE
  became THE TABLE, and the idle-panel signatures changed.
- **Reviewed (targeted, 2026-10-18):** migrations 004 and 005 and their
  backfills over live rooms, `db::rearrange` and every seat write, the
  seat routes, and the two new 3 Man transitions. Two fixes came out of it:
  - ee60102: seat writes use `BEGIN IMMEDIATE`. A deferred transaction
    panicked on `SQLITE_BUSY_SNAPSHOT` when a drink tap committed between
    its read and its write.
  - 2586b5b: join-order ties break by `rowid`, so friends who join in the
    same second don't swap seats at the next re-space.

  `game_draws.ring_slot` was checked by hand: nullable and visual only, and
  duplicate taps resolve to the lowest slot still on the table.

### After handover

- **9d242ad:** the smoke load caught Ring of Fire's big-screen felt
  rendering zero pixels tall (the panel was content-sized). It now fills
  the pane like 3 Man's. Full gate re-run on 9d242ad: exit 0, VERIFY OK,
  1160 tests.

