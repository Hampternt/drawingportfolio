# Drinks redesign + 3 Man — design spec

Date: 2026-07-30
Status: reviewed (adversarial design review round 1 folded in), pending user approval
Source of truth for visuals: Claude Design project `33b5226e-714a-4913-8099-6cfbd3847b05`
("Drinks - Redesign.dc.html", "Drinks - 3 Man.dc.html", `sounds/README.txt`).
Local reference copies: session scratchpad `redesign.html`, `3man.html`.

## Goal

Replace the `/drinks` UI with the phone-first redesign (new visual language,
three-tab room, thumb-zone drink bar, real spectator layout, QR join, who's
here, house rules from Jacks, end-of-night summary, sounds/emotes) and add a
second game, **3 Man** (two dice, live seating order, doubles give-away flow).

Two phases, same branch of work:

- **Phase 1** — redesigned shell + Ring of Fire on it, verified in a browser.
- **Phase 2** — 3 Man on top of the new shell.

## Decided rules (user-confirmed)

| Question | Decision |
|---|---|
| First 3 Man | Picked by hand at start (defaults to starter); reassignable any time from the TABLE tab |
| Hand-back after a 3 Man rolls a 3 | May give the title to anyone except themselves (no lock-out) |
| Double 3s | Each 3 counts (3+3 = two drinks for the 3 Man, plus the doubles flow still fires) |
| Gifted die rolls | Drink-what-you-roll only; 3/7/9/11 do NOT trigger on gift rolls |
| Dice passing | Explicit "PASS TO <name>" button, no auto-pass |
| Roll access | Any room member can trigger the roll (dead-phone insurance); the roll is always attributed to the seat whose turn it is |
| Verdict drinks | Auto-logged: the app inserts real `drink` events for the victim (undo-able via their UNDO) |

### Actor gating (per action — review finding)

| Action | Who may trigger |
|---|---|
| `/tm/roll` | any member (attributed to current seat) |
| `/tm/gift-roll` | any member (attributed to the gift's victim) |
| `/tm/pass` | any member (dead-phone insurance) |
| Hand-off pick (`/tm/three-man` during HandOff) | current roller only |
| `/tm/mode`, `/tm/target`, `/tm/clear-slot`, send | double owner only |
| Seat reorder / TABLE-tab 3 Man reassign | any member |
| Start / end game, end night | any member (matches Ring of Fire today) |

Server enforces these; other phones render spectator text ("X is picking…").

## Architecture

Keep the existing pattern end-to-end: handlers mutate SQLite → re-render
HTML fragments server-side (`render.rs`) → broadcast over the per-room SSE
hub → clients swap `innerHTML`. The prototype's client-side game logic is a
simulation of this server loop and is NOT ported to the browser. Vanilla JS
only (no new client deps).

### Personalization contract (review finding — replaces "no client state")

Broadcast fragments are identical for every viewer; anything per-viewer is
resolved client-side by one generic pass, `personalize(root)`, run on
`DOMContentLoaded` and after every SSE swap (generalizing today's
`revealUseButtons`). Mechanisms, all driven by `document.body.dataset.playerId`:

- `[data-show-player="ID"]` / `[data-hide-player="ID"]` — show/hide blocks
  (USE buttons, ROLL THE DICE vs "X is rolling", hand-off picker vs
  spectator banner, assign flow vs "X is handing out", Jack rule input).
- `[data-me-text="…"]` — swap text when the referenced id is me
  ("You drink" vs "bob drinks", "YOU ROLLED" caption, "your rule" byline).
- Standings/leaderboard rows carry `data-player-id`, `data-drinks`,
  `data-shots`, `data-rank`; `personalize` highlights my row and copies my
  counts into the thumb-bar labels ("6 tonight") and the idle "YOUR NIGHT SO
  FAR" card (which is otherwise static markup in the fragment).
- 3 Man exposure line ("You're on their left — a 7 is yours."): the seat
  strip carries `data-order` (player ids), `data-roller`, `data-three-man`;
  a tiny client function derives the viewer's exposure text. No game logic
  beyond neighbour lookup lives client-side.

Client state remains: tab selection, mute flag, animation keys, and this pass.

### SSE protocol

Today: `leaderboard`, `game`, `ended`. New set:

- `leaderboard` — standings rows (phone STANDINGS tab + big screen right pane).
- `game` — phone GAME-tab panel.
- `screen` — spectator main pane (phone and screen markup differ; each
  surface listens only to its own event).
- `room` — ROOM/TABLE-tab content plus a small top-bar strip (member chips,
  "N here"/"N at the table", 3 MAN chip) the client copies into the shell's
  top bar; carries `data-mode="idle|ring_of_fire|three_man"`, from which the
  client renames tab 3 (ROOM ↔ TABLE). Broadcast on join / rule added /
  king drawn / seat or 3 Man change / game start-end.
- `emote` — `{glyph}` broadcast when a self-logged drink/shot lands; ALL
  surfaces (including the origin phone) float it from the broadcast only —
  no local float, so no double. Sounds play only on the tapping phone.
  Auto-logged verdict drinks do NOT fire emotes.
- `ended` — unchanged.

On stream connect (incl. EventSource auto-reconnect) the server front-loads
a snapshot of every **stateful** kind: `leaderboard`, `game`, `screen`,
`room`. Transient kinds (`emote`, announcements) are never snapshotted and
are lossy by design. Hub channel capacity raised 32 → 128 (more kinds per
mutation; a lagged receiver only loses transients it could afford to lose).

### Concurrency (review finding)

3 Man mutations are load → transition → persist on `games.state_json`. Two
phones may act simultaneously ("anyone can roll"), so `GameState` gains a
per-room `tokio::sync::Mutex` map; every `/tm/*` handler holds the room's
lock across load-transition-persist (drink-event inserts included). Phase
guards inside the lock turn the loser of a race into a 409 fragment, not a
double-applied roll. (Ring of Fire keeps its existing UNIQUE-constraint
guard; no lock needed.)

### Data model — migration `003_shell_and_three_man.sql` + code guards

- `games.kind TEXT NOT NULL DEFAULT 'ring_of_fire'` (`ring_of_fire` |
  `three_man`). `ALTER TABLE ... ADD COLUMN` is not idempotent in SQLite, so
  `run_migrations()` guards each ALTER with a `PRAGMA table_info` check.
- `games.state_json TEXT` — 3 Man state snapshot; NULL for Ring of Fire.
  For 3 Man games `deck_order` and `rules_json` are `''`.
- `game_draws.rank INTEGER` — the drawn card's rank, written on insert;
  backfilled once for existing rows in `run_migrations()` (parse each
  game's `deck_order`, `WHERE rank IS NULL` makes it idempotent). Makes
  king-count and lifetime King's Cups plain SQL.
- New table `game_house_rules (id, game_id, draw_id UNIQUE, player_id,
  text, created_at)` — rules typed after drawing a Jack. `draw_id` ties a
  rule to its Jack draw (one rule per Jack, server-verifiable).
- The existing partial unique index (one active game per room) also
  guarantees Ring of Fire and 3 Man can't run simultaneously.
- **Cross-kind guards** (review finding): every RoF route
  (`/game/draw|spend|rule|end`) rejects `kind != 'ring_of_fire'` with the
  409 fragment; every `/tm/*` route rejects `kind != 'three_man'`. Tested.
- Ending a room (manual end + idle sweep) also ends its active game — no
  orphaned `ended_at IS NULL` game rows.
- `GameError::NoActiveGame` message reworded (no longer names Ring of Fire).

### New lifetime-stat queries

Landing start-or-join card: lifetime drinks, shots (existing), nights
(`COUNT(DISTINCT room_id)` from `room_players`), King's Cups
(`COUNT(*) FROM game_draws WHERE player_id = ? AND rank = 13` — enabled by
the new `rank` column).

## Phase 1 — shell + Ring of Fire

### Templates & CSS

- `game.css`: rewrite. Same palette (`#0b0910`/`#17141f` surfaces, `#b48ef7`
  violet, `#ffb570` amber, `#f7768e` red, `#f2eef8`/`#cdc6dd`/`#8d87a0`
  text), Archivo (display, 500–900) + Space Grotesk (UI, 400–700)
  **self-hosted**: woff2 files committed under `drinkinggame/assets/fonts/`,
  embedded with `include_bytes!`, served from `/assets/fonts/*` — no
  third-party requests. Keyframes from the prototypes (flipA/B, popA/B,
  livePulse, floatUp, floatUpBig; tumbleA/B for Phase 2).
- **Animation keying** (review finding): fragments embed
  `data-anim-key` (RoF: draw count + spend count; 3 Man: roll/bump count).
  The client compares the incoming key with the container's previous key and
  re-applies the animation class only when it changed — unrelated broadcasts
  (announcements, house-rule adds) don't re-flip the hero card or re-tumble
  dice. Alternating A/B keyframe names give replays.
- `room.html`: three-tab shell. Top bar (room-code pill with live dot,
  top-bar strip container filled by the `room` fragment, mute toggle
  persisted in localStorage — one global key, deliberately shared across
  rooms). Tabs GAME / STANDINGS / ROOM(→TABLE) switch client-side; SSE keeps
  all three fresh. Fixed bottom bar: +1 DRINK / +1 SHOT / UNDO with personal
  tonight-counts (via personalize pass). Float-up emote layer above the bar.
- GAME tab states (server-rendered fragments):
  - *idle*: "your night so far" stat card + Ring of Fire start card (preset
    select + START + presets link). Phase 2 adds the 3 Man start card.
  - *active*: announcement banner (transient, broadcast-only), deck progress
    bar + N LEFT, hero card (rank/suit + rule title/text + drawer label,
    flip on anim-key change), TAP TO DRAW, IN HAND strip (holder label; USE
    revealed per personalization contract), End game early.
  - *over* (transient broadcast, like today's summary): Game over header,
    HARDEST HIT card, superlatives grid (most draws, most shots, room total,
    King's Cup), surviving house rules, DEAL A NEW DECK (idle panel below).
- Jack flow: when the latest draw is a Jack, the hero card includes an
  inline rule input + SET, revealed only on the drawer's phone
  (`data-show-player`). `POST /room/{code}/game/rule` (text ≤ 200 chars).
  **Server guard**: latest draw of the active game is rank 11 ∧ poster is
  its drawer ∧ no rule exists for that `draw_id`; else 409 fragment.
  On success: insert, broadcast `room` + announcement "hampus made a rule".
- ROOM tab: room code card (SHARE LINK via `navigator.share`/clipboard,
  OPEN BIG SCREEN link), WHO'S HERE grid (room members; dot = membership,
  not liveness), HOUSE RULES list, King's Cup fill (kings drawn / 4), End
  the night (confirm).
- `screen.html`: 1280×720 spectator. Left: `screen` fragment — idle "Just
  drinking." / active hero card at display scale + HELD RIGHT NOW / over
  "X lost." + superlatives. Right: JOIN header (code + QR), standings rows
  scaled to fill, footer (King's Cup fill + house-rules line).
- `landing.html`: redesigned login (name/PIN/LET'S GO) and start-or-join
  (EVENING <NAME>, lifetime stats headline, START A NIGHT, join-code card).
- `presets.html`, `preset_edit.html`, `error.html`: restyled by the shared
  CSS; structure unchanged.

### Server changes

- `render.rs`: rewritten fragment builders; `game` (phone) and `screen`
  (spectator) variants; `room` fragment builder; summary/superlatives.
- `game.rs`: broadcast both variants; house-rule handler; king count and
  superlatives from `game_draws` (now with `rank`) + `events`.
- `routes.rs`: `POST /room/{code}/game/rule`; sounds + fonts routes; emote
  broadcast in `log_event`; `room` broadcast on join; SSE snapshot set.
- **QR join round-trip** (review finding): QR encodes the absolute room URL.
  `PlayerSession`-guarded room pages redirect unauthenticated visitors to
  `{base}/?next=/room/CODE`; the landing login form carries `next` through
  as a hidden field and login redirects to it (validated: must match
  `^{base}/room/[A-Z]{4}$`), landing straight into the room. Origin derived
  from `X-Forwarded-Proto` + `Host`, falling back to `https` for non-local
  hosts. **Ops step**: add `proxy_set_header X-Forwarded-Proto $scheme;` to
  the manually-deployed nginx config (deploy note).
- Sounds: served from a working-directory-relative `drinks-sounds/` dir
  (overridable via `DRINKS_SOUNDS_DIR`, added to `.env.example`), route
  `/assets/sounds/{name}` with filename allowlist
  (`drink|shot|card-draw|card-use|dice-roll|dice-give`, `.mp3`).
  Missing file → 404; client `Audio.play().catch(()=>{})` keeps taps
  silent until files are dropped in. No mp3s committed.

## Phase 2 — 3 Man

### Engine (`three_man.rs`, pure + serde)

State (serialized into `games.state_json`):

```
order: Vec<player_id>        // clockwise seating; position 0 rolls first
roller_idx: usize
three_man: player_id
phase: Ready | Rolled | HandOff | Assign | Gifts
dice: Option<(u8, u8)>
calls: Vec<Call { player_id, amount, reason }>
double: Option<Double { value, owner, mode: Option<Both|Split>,
                        slots: Vec<Option<player_id>>,
                        gifts: Vec<Gift { player_id, dice_count, values: Option<Vec<u8>> }>,
                        payback: Option<String> }>
pending_double: bool         // handoff resolves first, then assign
handoff_note: Option<String>
last_roller: Option<player_id>
stale: bool                  // verdict stays rendered after pass, dimmed
```

Seeding at `/tm/start`: `order` = room members by `joined_at`, rotated so
the starter is position 0; starter is initial 3 Man. **Minimum 2 players**;
start rejected below that.

Transitions (pure functions, unit-tested):

- `roll(rng)` from `Ready`: each 3 on a die + a 3-total each count 1
  against the 3 Man — unless the roller IS the 3 Man → `HandOff`, no drink;
  sum 7 → left neighbour (`order[(i+1) % len]`, "next to roll"); sum 9 →
  right neighbour; sum 11 → roller. At 2 players left == right == the other
  player (accepted). Doubles → `Assign` (after hand-off resolution when both
  fired, via `pending_double`). Otherwise `Rolled`.
- `give_three_man(target)` from `HandOff`: target ≠ current 3 Man; then
  `Assign` if `pending_double` else `Rolled`.
- `set_mode(Both|Split)` from `Assign`: Both = 1 slot (victim rolls 2 dice),
  Split = 2 distinct slots, owner excluded. **Split unavailable when
  `order.len() < 3`** (hidden client-side, rejected server-side).
- `pick_target` / `clear_slot` / `send` → `Gifts`.
- `gift_roll(slot, rng)` from `Gifts`: victim drinks the rolled total; when
  all gifts rolled, if any gifted die == double value → payback: owner
  drinks the combined total of all gifted dice.
- `pass()` from `Rolled` / `Gifts`-complete: `roller_idx` advances left;
  verdict stays rendered, `stale` (dimmed, "LAST ROLL · name").
- `move_seat(player, ±1)`, `set_three_man(player)`: any time from TABLE tab
  (roller identity preserved by index fix-up).
- Mid-game joins: `room_page`'s join hook appends the new member to `order`
  (under the room lock) and broadcasts `room`.

**Auto-log granularity** (review finding): a call of amount N / gift total
of T inserts N (resp. T) individual `'drink'` event rows for the victim —
the leaderboard counts rows. One leaderboard broadcast per resolution, not
per row. No emotes for auto-logged drinks. UNDO caveat (accepted +
documented): the victim's UNDO tombstones their latest event, whichever it
is — undoing a 4-drink gift is four taps, and a self-logged drink in
between is undone first.

### Routes

`POST /room/{code}/tm/start`, `/tm/roll`, `/tm/three-man`, `/tm/mode`,
`/tm/target`, `/tm/clear-slot`, `/tm/gift-roll`, `/tm/pass`, `/tm/seat`,
`/tm/end`. All guarded by `member_room` + kind check + actor gating table +
phase guard, executed under the per-room lock; invalid → 409 fragment.

### UI

- Phone GAME tab: turn banner ("YOUR TURN" pulse, via personalize), seat
  strip (clockwise, "LEFT = NEXT TO ROLL" caption; **one tag per seat with
  precedence ROLLING > ←7 > 9→ > 3 MAN**, border colors same precedence),
  verdict card (pip-grid dice + tumble on anim-key change, big sum, call
  rows "X drinks N — reason" with "You drink" me-swap, "Nobody drinks"
  dashed box), hand-off picker (roller's phone only; others see the banner),
  doubles flow (owner's phone only: mode choice → slots + target grid →
  SEND THE DICE), gifts list (ROLL button on each pending gift — any
  member's phone shows it, attributed to the victim), payback banner, ROLL
  THE DICE / "X is rolling — <derived exposure line>", PASS TO <name>.
  Top-bar strip gains the amber "3 MAN <name>" chip and "N at the table".
- Phone STANDINGS tab: rows carry the 3 MAN badge on the holder (and data
  attrs per the personalization contract).
- Phone TABLE tab (tab 3 renamed while 3 Man runs): seating list with ↑/↓
  (44px targets) and per-row "3 MAN" assign, THE RULES reference cards
  (3/7/9/11/==, static text v1), End the night. **Deliberate deviation from
  the prototype**: the room-code card and WHO'S HERE grid stay above the
  seating list (otherwise share-link/big-screen are unreachable mid-game).
- Big screen: left pane — 3 MAN header (holder chip, waiting label), giant
  dice + sum + reason headline, call rows at display scale, hand-off /
  double / gifts / payback banners; the full-pane "WAITING ON <name>" state
  appears **only before the first roll** — between rolls the dimmed stale
  verdict stays (prototype `screenWaiting: !dice`). Right pane — join + QR
  + standings (3 MAN badge). Full-width bottom seat strip ("7 hits the
  left · 9 hits the right · 11 hits the roller").
- Idle GAME tab offers both start cards: Ring of Fire (violet) and 3 Man
  (amber, one-line explainer).
- Sounds: `dice-roll.mp3`, `dice-give.mp3` via the same route.

## Testing

- Unit: `three_man.rs` transitions (verdict combinations incl. double 3s,
  3-total, hand-off + pending double, split dedupe + <3-player rejection,
  payback math, seat move/index fix-up, pass wrap-around, stale flag);
  migration idempotence (run twice, incl. rank backfill); db queries (house
  rules + draw_id uniqueness, nights/kings lifetime counts).
- Integration (`tests/http.rs`): new routes happy-path + guards (non-member
  403, wrong-phase/wrong-kind/wrong-actor 409/403, Jack-rule guard), SSE
  snapshot set on connect, `next` round-trip on login.
- Existing render/route tests updated for new fragment markup.
- Manual: real-browser session with two phone windows + one screen window
  per CLAUDE.md verification rules.

## Verification

`cargo fmt --check`, `cargo clippy`, `cargo test` (workspace) all green;
browser walkthrough: login → start night → tabs live via second window →
Ring of Fire full game incl. Jack rule + summary → 3 Man full game incl.
hand-off, both doubles modes, payback, seat reorder, mid-game join → end
night. Deploy note: nginx `X-Forwarded-Proto` header (manual step).

## Out of scope

- Editing 3 Man rule text / presets for 3 Man.
- Removing players from a room or seat list.
- Live "connected right now" presence (dot = room membership).
- Committing actual mp3 files (drop-in directory + README only).
- Portfolio (`base.html`) surfaces — `/drinks` templates stay standalone
  (recorded exception).
