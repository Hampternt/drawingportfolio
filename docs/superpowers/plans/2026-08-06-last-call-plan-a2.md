# Last Call — Plan A2: the game wiring

> **For agentic workers:** REQUIRED SUB-SKILLS: `plan-economics` (this repo's
> task classes, review policy and plan sizing) then
> superpowers:subagent-driven-development to execute task-by-task.

**Goal:** Make `last_call` a startable third game kind, give it its own phone
shell at its own route, and feed that shell each viewer's own hand over a
signal-and-fetch SSE contract that no other player — and no spectator — can read.

**Architecture:** `last_call.rs`'s pure state machine round-trips through the
existing `games.state_json` column, so **this plan adds no migration** (spec
§3.2: `games.kind` is `TEXT NOT NULL DEFAULT 'ring_of_fire'` with no `CHECK`).
Every `/lastcall/*` handler follows `tm_routes.rs` exactly — resolve the room,
take its `RoomLocks` guard, re-load state under the guard, mutate, persist and
broadcast before the guard drops. Public fragments render from `PublicView`
(spec §3.4); the viewer's hand is fetched by the viewer over
`GET /room/{code}/lastcall/hand`, a route that takes no player identifier at all
(spec §6.1).

**Slice:** Plan A2 of four for slice 1 (spec §10), in the order **A → A-vis →
A2 → B**. Its immediate predecessor is **Plan A-vis**, not Plan A: by the time
this plan starts, the component library has not only been built and unit-tested
(Plan A) but *seen* on the `/lastcall/preview` gallery (Plan A-vis), which is
where a wrong token or text rule was meant to be caught. This plan **consumes
both** — `LastCallState`, `PublicView`, `lc_cards`, `preview_state()`,
`lastcall.css` and every `lc_render` builder from Plan A; `lc_motion.js`,
`window.lcFlight` / `window.lcAnchor` and the flight layer from Plan A-vis — and
adds no visual vocabulary of its own.

When it is done: a room member starts a Last Call game from the room's idle
panel, everyone who opens the room link lands in the Last Call shell instead of
the Ring of Fire one, each player registers what they are drinking and is dealt a
placeholder hand, and each phone sees **its own** hand as a vertical list of
CardFaces that repaints when the room's state changes.

**The four-plan chain, end to end:**

1. **Plan A — the component library** *(done)*. Types, `PublicView`, the
   adversarial catalog, `preview_state()`, `lastcall.css`, and `lc_render.rs`'s
   components to the §7.8 contract. Nothing in it was viewable.
2. **Plan A-vis — motion and the style guide** *(done)*. The §7.7 motion library
   and flight helper, and the permanent `GET /lastcall/preview` gallery.
3. **Plan A2 — the game wiring** *(this plan)*. Every Class C task in the slice.
4. **Plan B — the felt surfaces.** `lc_screen.html`, the D.2 seat-ring angle
   layout that positions Plan A's plaques around Plan A-vis's felt, the
   `/room/{code}/screen` kind branch, `GET …/lastcall/table`, and the F.3 mini
   table. **Plan B assembles components and authors none** — §7.6's
   component/positioning split already put the plaque, hand strip, deck stack
   and discard slot in Plan A.

**Every task here is Class C**, which is the point of the ordering: spec §10
names exactly three — the session-gated private hand route, the SSE contract and its
stale-drop rule, and the room entry redirect that branches on active-game kind
and must not disturb Ring of Fire or 3 Man. Plan A carried the Class A/B work.

**Size note.** Three tasks is under `plan-economics`'s 4–6 guide, deliberately:
each carries a task reviewer on a capable model, and Task 1 is the heaviest in
the series. If a session runs long, Task 1 splits cleanly at Step 4 (everything
before it is game-kind registration and setup; the redirect and the late-join
hook are the second half) — but it is written as one task because the cross-game
arms and the redirect are the same invariant, and splitting them would ship a
first half that panics.

---

## Global Constraints

Every task's requirements implicitly include this section.

### What Plan A and Plan A-vis already produced — build against these, do not re-derive

```rust
// last_call.rs
Deck { Beer, Cider, Wine, Liquor, Soft }   // ::ALL, .pulls(), .slug(), .label(), ::from_slug()
Beat { Draw, Deal, Diplomacy, Lock, Reveal, Resolve }  // ::ORDER, .index(), .label(), .slug(), .hue(), .next()
Status { Alive, Eliminated }
LcError { NotSeated, BadHandicap, NotImplemented }
STARTING_HP: i32 = 15;  MAX_SEATS: usize = 8;
HANDICAP_MIN_PCT: u16 = 25;  HANDICAP_MAX_PCT: u16 = 300;
pub fn pull_cost(cost: u8, handicap_pct: u16) -> u8;      // rounds UP

impl LastCallState {
    pub fn new(members: Vec<(i64, String)>, rng_seed: u64) -> Self;
    pub fn to_json(&self) -> String;
    pub fn from_json(s: &str) -> Self;                     // EXPECTs — see below
    pub fn seat_of(&self, player_id: i64) -> Option<usize>;
    pub fn add_player(&mut self, player_id: i64, name: &str);
    pub fn set_vessel(&mut self, player_id: i64, deck: Deck, container: &str) -> Result<(), LcError>;
    pub fn set_handicap(&mut self, target_id: i64, handicap_pct: u16) -> Result<(), LcError>;
    pub fn public_view(&self) -> PublicView;
}

// lc_render.rs — every builder Plan A shipped
pub fn card_face(card: &Card) -> String;
pub fn card_face_expanded(card: &Card) -> String;
pub fn card_pip(card: &Card) -> String;
pub fn card_mini(card: &Card) -> String;
pub fn card_back(deck: Deck, size: BackSize) -> String;
pub fn card_dot(deck: Deck) -> String;
pub fn player_plaque(seat: &PublicSeat) -> String;
pub fn hand_strip(decks: &[Deck], n: usize) -> String;
pub fn deck_rule(decks: &[Deck]) -> String;
pub fn deck_stack(deck: Deck, count: u16) -> String;
pub fn discard_slot(count: usize) -> String;
pub fn lc_banner(view: &PublicView) -> String;
pub fn beat_timer(duration_ms: u32, elapsed_ms: u32) -> String;
pub fn lc_public_panel(view: &PublicView) -> String;

// assets: /assets/lastcall.css from Plan A, /assets/lc_motion.js from Plan A-vis
GET /assets/lastcall.css      GET /assets/lc_motion.js
window.lcFlight(fromEl, toEl, opts)   window.lcAnchor(name, root)

// the shared runtime fixture builder (spec §8) — NOT #[cfg(test)]. Plan A
// defines it, Plan A-vis renders it; this plan does not need it, but any
// test here that wants a realistic eight-seat state should call it rather
// than hand-rolling one.
pub fn preview_state() -> LastCallState;
```

**`from_json` `expect`s and `""` is not valid JSON.** Every writer in this plan
must therefore pass `Some(&st.to_json())` to `db::start_game`, never `None` —
every reader does `from_json(game.state_json.as_deref().unwrap_or_default())`.

### The §7.8 component contract — Plan A shipped the markup, this plan selects on it

This plan adds exactly one new component, the private hand region, and it must
satisfy the same contract:

| Component | Root | Requires | Exposes | Motion anchor | Filled by |
| --- | --- | --- | --- | --- | --- |
| Hand region | `#lc-hand` | `data-seq` | `data-count` | `hand` | `GET …/lastcall/hand` (this plan) |

Everything else — `.lc-cardface[data-card-id]`, `.lc-plaque[data-seat]`,
`.lc-deckstack[data-deck]`, `#lc-banner`, `#lc-felt`, `#lc-flights` and the rest
— already exists. **Select on the contract's attributes; do not invent new
ones.** A selector this plan needs that Plan A did not ship is a bug report
against Plan A, not a licence to add markup here.

This plan *is* where behaviour arrives: `hx-post` paths, form actions and the
`EventSource` are all legitimate here and were forbidden in Plan A.

### Repo rules that bind here

- **SQL lives in `db.rs` only.** Nothing in this plan needs a new db function —
  `start_game()`, `get_active_game()`, `set_game_state()`, `end_game()`,
  `room_members()`, `is_room_member()`, `get_open_room()` and `touch_room()`
  already exist and are the complete data layer. If a task believes it needs raw
  SQL in a handler, it has misread the plan.
- **No migration.** Do not write one. `games.kind` accepts `last_call` today.
- **`cargo sqlx prepare` is not needed** — the `drinkinggame` crate uses
  runtime-checked sqlx queries and has no `.sqlx` cache entries (CLAUDE.md).
- **The crate's templates do not extend `base.html`** — a recorded exception.
  `lc_room.html` is standalone, exactly like `room.html`.
- **No `<style>` blocks in templates.** Any new rule goes in
  `assets/lastcall.css` under a named section comment. Never nest `/*` inside a
  CSS comment.
- **JS lifecycle.** `lc_room.html` is standalone with **no `hx-boost`**, so its
  inline script runs once at the end of `<body>` and needs no `DOMContentLoaded`
  binding — matching `room.html`, whose script does the same. The
  `DOMContentLoaded` + `htmx:afterSwap` + double-injection-guard rule applies to
  modules under `drinkinggame/assets/`; `lc_motion.js` already follows it.
- **`palette.js` / `base.html` nav are not touched.** Those apply to new
  *portfolio* sections; Last Call lives inside the already-registered `/drinks`
  mount.

### Routes added by this plan

Written unprefixed — `nest_service` strips the `/drinks` mount, and only
*generated URLs* interpolate `base_path`.

| Method | Path | Task |
| --- | --- | --- |
| POST | `/room/{code}/lastcall/start` | 1 |
| POST | `/room/{code}/lastcall/vessel` | 1 |
| POST | `/room/{code}/lastcall/handicap` | 1 |
| GET | `/room/{code}/lastcall` | 2 |
| GET | `/room/{code}/lastcall/hand` | 2 |

`GET /room/{code}/lastcall/table` and the `/room/{code}/screen` kind branch are
**Plan B**, with the surfaces they serve.

**Verification for every task:** `./scripts/verify.sh` — all green, output
quoted in the report.

**Browser checkpoints:** after **Task 2** (the shell is first browsable) and
after **Task 3**, before the final review. Not per task.

---
### Task 1: Game-kind wiring, setup routes, entry redirect and the cross-game arms

**Class:** C (logic tests cannot encode — reviewer required)

**Why this class:** a cross-game invariant. `GET /room/{code}` must redirect to
the Last Call shell **only** when the active game is `last_call`, and must leave
Ring of Fire and 3 Man — including the mid-game 3 Man join hook and its lock
discipline — byte-for-byte unchanged. And the three `game.rs` panel builders
currently fall through to the Ring of Fire branch for any unknown kind, which
for a `last_call` game **panics** (Step 1). No test enumerates the set of places
that assume "not three_man ⇒ ring_of_fire"; a reviewer reading the diff against
`game.rs` and `routes.rs` is what finds the one that was missed.

**Files:**
- Create: `drinkinggame/src/lc_routes.rs`
- Modify: `drinkinggame/src/lib.rs` (`pub mod lc_routes;`)
- Modify: `drinkinggame/src/game.rs` — `current_panel`, `current_screen_panel`,
  `current_room_panel` (add a `last_call` arm to each)
- Modify: `drinkinggame/src/render.rs` — `game_idle_panel` (third start card)
  plus three small placeholder panel builders
- Modify: `drinkinggame/src/routes.rs` — `room_page` (the redirect), `router()`
  (three route registrations)
- Test: `drinkinggame/tests/http.rs`

**Interfaces:**
- Consumes: `LastCallState::{new, from_json, to_json, add_player, set_vessel,
  set_handicap, seat_of, public_view}`, `Deck::from_slug`, `LcError`,
  `HANDICAP_MIN_PCT`/`HANDICAP_MAX_PCT` (**Plan A**);
  `lc_render::lc_public_panel` (**Plan A**); existing
  `crate::game::member_room(&GameState, &str, &Player) ->
  Result<Room, Response>`, `db::{start_game, get_active_game, set_game_state,
  room_members, touch_room}`, `RoomLocks::for_room`.
- Produces:

```rust
// lc_routes.rs
pub(crate) struct LcCtx { pub room: Room, pub game: Game, pub st: LastCallState }

/// member_room -> active game -> kind == "last_call" else WrongGameKind ->
/// parse state. The shared entry point for every `/lastcall/*` handler, in
/// the exact shape of `tm_routes::load_tm`.
pub(crate) async fn load_lc(
    state: &GameState, code: &str, player: &Player,
) -> Result<LcCtx, axum::response::Response>;

/// set_game_state -> touch_room -> broadcast. Task 3 adds the LcPublic /
/// LcTick publishes to this body; the signature does not change.
pub(crate) async fn persist_and_broadcast_lc(state: &GameState, ctx: &LcCtx);

pub async fn lc_start_handler(State<GameState>, PlayerSession, Path<String>) -> Response;
pub async fn lc_vessel_handler(State<GameState>, PlayerSession, Path<String>, Form<VesselForm>) -> Response;
pub async fn lc_handicap_handler(State<GameState>, PlayerSession, Path<String>, Form<HandicapForm>) -> Response;

#[derive(Deserialize)] pub struct VesselForm { pub deck: String, pub container: String }
#[derive(Deserialize)] pub struct HandicapForm { pub target: i64, pub handicap_pct: u16 }
```

- [ ] **Step 1: The cross-game arms — do this first, it is a live panic**

`cards::parse_deck("")` splits to `[""]`, `Card::from_code("")` returns `None`,
and `parse_deck` calls `.expect("corrupt deck_order in db")`. A `last_call` game
is started with `deck_order = ""` (a Ring of Fire concept 3 Man also leaves
empty), and every one of these matches falls through to the Ring of Fire branch:

```rust
// game.rs, current_screen_panel — and the same shape in current_panel
Some(game) if game.kind == "three_man" => { … }
Some(game) => active_screen_panel(state, &game, code).await,   // <- panics for last_call
None => render::screen_panel_idle(code),
```

So `GET /room/{code}/sse` on a Last Call room would 500 before a single frame.
Add an explicit arm **above** the catch-all in all three of `current_panel`,
`current_screen_panel` and `current_room_panel`:

```rust
Some(game) if game.kind == "last_call" => { … }
```

- `current_panel` → `render::lc_placeholder_panel(&state.base_path, code)`: a
  one-line panel with a link to `{base_path}/room/{code}/lastcall`. Nobody
  should normally see it (the redirect in Step 4 sends them straight there),
  but the SSE snapshot renders it on every connection.
- `current_screen_panel` → `render::lc_screen_placeholder(code)`: "LAST CALL —
  the big screen lands next slice." **Plan B replaces this** with the
  `lc_screen.html` branch.
- `current_room_panel` → the existing builder with `mode = "last_call"`, empty
  house rules, `kings = 0`, no seating. Read the function first; keep its
  existing shape and only add the arm.

The `render.rs` builders are two `format!` one-liners; put them next to
`screen_panel_idle`.

- [ ] **Step 2: `lc_routes.rs` — `load_lc` and `persist_and_broadcast_lc`**

Copy `tm_routes.rs`'s `load_tm` / `persist_and_broadcast` shape exactly,
substituting `"last_call"` and `LastCallState`. `persist_and_broadcast_lc` in
this task:

```rust
pub(crate) async fn persist_and_broadcast_lc(state: &GameState, ctx: &LcCtx) {
    db::set_game_state(&state.pool, ctx.game.id, &ctx.st.to_json()).await;
    db::touch_room(&state.pool, ctx.room.id).await;
    // Task 3 adds the LcPublic / LcTick publishes here. The room panel is
    // still broadcast so the standings/ROOM surfaces of any phone that is
    // still on the old shell stay current.
    crate::game::broadcast_room(state, ctx.room.id, &ctx.room.code).await;
}
```

- [ ] **Step 3: The three setup routes**

Each follows `tm_routes.rs`'s locking discipline **verbatim**: resolve the room,
take `state.locks.for_room(room.id)`, re-load state under the guard, mutate,
`persist_and_broadcast_lc`, all before the guard drops.

`lc_start_handler` mirrors `tm_start_handler`:
`member_room` → lock → `db::room_members` (`< 2` ⇒ `GameError::TooFewPlayers`)
→ `LastCallState::new(members.iter().map(|m| (m.id, m.name.clone())).collect(),
rng_seed)` → `db::start_game(&pool, room.id, "last_call", "", "",
Some(&st.to_json()))` → re-load via `load_lc` under the lock →
`persist_and_broadcast_lc`. `rng_seed` is
`rand::thread_rng().gen::<u64>()` — the *only* randomness in the feature, taken
in the route and stored, never generated inside `last_call.rs`.

**`state_json` must be `Some(&st.to_json())`, never `None`** — `from_json`
`expect`s and `""` is not valid JSON (Task 1, Step 1).

Returns `StatusCode::NO_CONTENT` like `tm_start_handler` (it is posted from the
idle panel's HTMX form with `hx-swap="none"`; the phone then follows the
redirect on its next navigation).

`lc_vessel_handler`: `Deck::from_slug(&form.deck)` — `None` ⇒
`StatusCode::UNPROCESSABLE_ENTITY`. Trim `container` and reject
`len() > 24` the same way. Then `ctx.st.set_vessel(player.id, deck, container)`,
mapping `LcError::NotSeated` ⇒ `GameError::NotYourCall`. Redirects to
`{base_path}/room/{code}/lastcall` on success (plain form post, not HTMX).

`lc_handicap_handler`: `ctx.st.set_handicap(form.target, form.handicap_pct)`.
Both `LcError::BadHandicap` and `LcError::NotSeated` map to
`StatusCode::UNPROCESSABLE_ENTITY` — here `NotSeated` means the *target* named
by the form is not in this game, which is a bad request, not a missing game.
**No ownership check:** any room member may set any player's handicap (spec §2,
item 2). Redirects like the vessel handler.

`u16` in `HandicapForm` is doing real work — it rejects negatives, decimals,
`NaN` and `inf` at extraction, so no non-finite value can ever reach the state
blob. This is spec §6.1's pattern (remove the input rather than check it)
applied to a scalar; note it in a comment.

- [ ] **Step 4: The entry redirect in `room_page`**

Read `routes.rs:153-214` before touching it. The order matters:

```rust
let code = code.to_uppercase();
let Some(room) = db::get_open_room(&state.pool, &code).await else { … };
db::join_room(&state.pool, room.id, player.id).await;   // room URLs are invite links

{
    let lock = state.locks.for_room(room.id);
    let _guard = lock.lock().await;
    let mut tm_joined = false;
    let mut last_call = false;
    if let Some(game) = db::get_active_game(&state.pool, room.id).await {
        if game.kind == "three_man" { …existing block, unchanged… }
        if game.kind == "last_call" {
            // Same mid-game-join problem 3 Man has: LastCallState seats
            // players in its own `players[]`, so a newcomer who opens the
            // room link needs seating there too or they have no hand and no
            // plaque. Same lock discipline — releasing before the broadcast
            // would let a concurrent handler's broadcast land first and
            // leave this request's stale render as the last word.
            let mut st = LastCallState::from_json(game.state_json.as_deref().unwrap_or_default());
            if st.seat_of(player.id).is_none() {
                st.add_player(player.id, &player.name);
                db::set_game_state(&state.pool, game.id, &st.to_json()).await;
            }
            last_call = true;
        }
    }
    crate::game::broadcast_room(&state, room.id, &code).await;
    if tm_joined { crate::game::broadcast_game(&state, room.id, &code, None).await; }
    if last_call {
        // Task 3 inserts `broadcast_lc(&state, room.id, &st).await;` here,
        // above the return and inside this guard.
        return Redirect::to(&format!("{}/room/{}/lastcall", state.base_path, code)).into_response();
    }
}
// unchanged from here: leaderboard, game panel, room panel, RoomTemplate
```

The redirect returns from **inside** the lock block, before the leaderboard and
panel renders — a Last Call room must not pay for (or risk) the Ring of Fire
render path on every entry. With no active game the room page is unchanged, and
starting a Last Call game from it is what creates the redirect condition.

- [ ] **Step 5: The third start card**

In `render.rs::game_idle_panel`, after the 3 Man card, add:

```rust
r#"<div class="start-card">
<h2 class="start-title">Last Call</h2>
<p class="start-sub">Register what you're drinking. Cards cost pulls of it. Six beats a round, out at 0 HP.</p>
<form hx-post="{base_path}/room/{code}/lastcall/start" hx-swap="none">
<button type="submit" class="btn-primary">START</button>
</form>
</div>"#
```

Then update `game_idle_panel`'s existing unit test (it asserts on the panel's
contents) to expect the new card, and its doc comment, which currently says
`.start-card-amber` is "reserved for the 3 Man start card (Task 11)".

- [ ] **Step 6: Register the routes**

In `router()`, grouped with the `/tm/*` block:

```rust
.route("/room/{code}/lastcall/start", post(crate::lc_routes::lc_start_handler))
.route("/room/{code}/lastcall/vessel", post(crate::lc_routes::lc_vessel_handler))
.route("/room/{code}/lastcall/handicap", post(crate::lc_routes::lc_handicap_handler))
```

- [ ] **Step 7: Tests in `tests/http.rs`**

Reuse the existing helpers: `test_app_with_pool`, `login`, `create_room`,
`room_page_html`, `post_form`, `get`, `body_string`.

1. `test_lastcall_sse_snapshot_does_not_panic` — **the regression from Step 1.**
   alice + bob, start a Last Call game, then `GET /room/{code}/sse` and assert
   `StatusCode::OK` and that at least one frame arrives. Without the `game.rs`
   arms this panics inside `parse_deck`.
2. `test_lastcall_start_requires_two_players` — a lone member gets
   `StatusCode::CONFLICT` and "needs at least 2 players".
3. `test_lastcall_start_rejects_non_member` — a third player who never opened
   the room gets `StatusCode::FORBIDDEN`.
4. `test_lastcall_routes_reject_rof_games` — start Ring of Fire, then
   `POST /room/{code}/lastcall/vessel` returns `StatusCode::CONFLICT` with
   "belongs to the other game" (the `WrongGameKind` path through `load_lc`).
   Mirror `test_tm_routes_reject_rof_games`.
5. `test_lastcall_vessel_sets_deck_constant_pulls` — register
   `deck=liquor&container=pint%20glass`; read `games.state_json` back from the
   pool and assert the seat's vessel is `pulls_max == 4` — the *deck's*
   constant, not anything derived from the (deliberately contradictory)
   container label. Also assert `hand.len() == 4`.
6. `test_lastcall_vessel_rejects_unknown_deck` — `deck=absinthe` returns
   `StatusCode::UNPROCESSABLE_ENTITY` and leaves `state_json` unchanged.
7. `test_lastcall_handicap_is_not_owner_scoped` — bob sets **alice's**
   handicap to 150 and it sticks (`303`/`SEE_OTHER`, state shows alice at 150).
   This is the spec §2 rule; a future "only you may set yours" regression fails
   here.
8. `test_lastcall_handicap_rejects_out_of_range` — `handicap_pct=301` and
   `handicap_pct=24` are `422`; `handicap_pct=-5` and `handicap_pct=abc` are
   `422` at extraction. State unchanged in all four cases.
9. `test_room_page_redirects_to_lastcall_shell` — after starting Last Call,
   `GET /room/{code}` returns `StatusCode::SEE_OTHER` with
   `location: /room/{code}/lastcall` (and `/drinks/room/{code}/lastcall` under
   `test_app_with_base("/drinks")`).
10. `test_room_page_unchanged_for_rof_and_three_man` — the invariant. With a
    Ring of Fire game active, `GET /room/{code}` is `200` and the body contains
    `data-pane="game"`; same for a 3 Man game; and with **no** active game it is
    `200` and contains all three start cards. No redirect in any of the three.
11. `test_room_page_seats_late_joiner_in_lastcall` — alice+bob start, then cara
    opens `/room/{code}`: she is redirected, and `state_json` now has three
    players with cara at `seat == 2`, `hp == 15`, `handicap_pct == 100`.
12. `test_lastcall_start_rejects_second_game` — starting Last Call in a room
    that already has an active game returns `StatusCode::CONFLICT`
    ("already running") via the `games` partial unique index.

- [ ] **Step 8: Commit**

```bash
git add drinkinggame/src/lc_routes.rs drinkinggame/src/lib.rs drinkinggame/src/game.rs \
        drinkinggame/src/render.rs drinkinggame/src/routes.rs drinkinggame/tests/http.rs
git commit -m "feat(drinks): start Last Call from a room, setup routes and the entry redirect"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

### Task 2: The F.1 phone shell and the private hand route

**Class:** C (logic tests cannot encode — reviewer required)

**Why this class:** session gating. `GET /room/{code}/lastcall/hand` is the
first route in this crate that returns state one player may see and another may
not, and spec §6.1 makes its authorization a *constraint* rather than a check —
the handler takes no player identifier of any kind, so "can player A fetch
player B's hand?" is unanswerable rather than merely guarded. That property is
verified by reading the handler signature, which is exactly what a test cannot
do. A reviewer must confirm the signature and that nothing downstream reintroduces
a caller-supplied identity.

**Files:**
- Create: `drinkinggame/templates/lc_room.html`
- Modify: `drinkinggame/src/lc_routes.rs` (`lc_page`, `lc_hand_handler`)
- Modify: `drinkinggame/src/routes.rs` (`router()` — two registrations)
- Test: `drinkinggame/tests/http.rs`

**Interfaces:**
- Consumes: `load_lc` / `LcCtx` (Task 1); `lc_render::{lc_banner, card_face,
  card_dot}` and the full CSS class contract from **Plan A**; `PlayerSession`
  from `crate::auth`.
- Produces — including two additions to `lc_render.rs` that Plan A deliberately
  left out, because they post to routes only this plan owns:

```rust
/// One row of the plain setup form: who, their handicap, their registered decks.
#[derive(Clone, Debug)]
pub struct SetupRow { pub player_id: i64, pub name: String, pub handicap_pct: u16, pub decks: Vec<Deck> }

/// The private hand fragment's body — the §7.8 "Hand region" component.
/// Not broadcast: served only to its own viewer by
/// `GET /room/{code}/lastcall/hand`.
pub fn lc_hand_pane(
    base_path: &str, code: &str, me: i64,
    hand: &[Card], rows: &[SetupRow], seq: u64,
) -> String;
```


```rust
#[derive(Template)]
#[template(path = "lc_room.html")]
struct LcRoomTemplate {
    base_path: String,
    code: String,
    player_id: i64,
    banner: String,       // lc_render::lc_banner(&view)
    hand_pane: String,    // lc_render::lc_hand_pane(...)
    seq: u64,
}

/// GET /room/{code}/lastcall — the F.1 phone shell.
pub async fn lc_page(State<GameState>, PlayerSession, Path<String>) -> Response;

/// GET /room/{code}/lastcall/hand — PRIVATE.
///
/// Takes no player identifier of any kind: no path segment, no query
/// parameter, no form field. The viewer's identity comes from the session
/// cookie alone, via `PlayerSession`. Written this way, "can player A fetch
/// player B's hand?" is unanswerable rather than merely guarded, and a
/// reviewer can verify it from this signature. Binding on every future
/// private fragment (spec §6.1).
pub async fn lc_hand_handler(
    State(state): State<GameState>,
    PlayerSession(player): PlayerSession,
    Path(code): Path<String>,
) -> axum::response::Response;
```

- [ ] **Step 1: `lc_room.html` — the shell, fixed vertical order**

Standalone, like `room.html` — it does **not** extend `base.html` (recorded
crate exception) and carries **no `hx-boost`**. Links only
`{{ base_path }}/assets/lastcall.css`. No `<script src=…htmx…>`: nothing in
this shell uses HTMX (the setup forms are plain posts), so do not link it.
**The htmx/no-htmx distinction is per-template, not per-plan** — Task 1's start
card is an `hx-post` form inside `room.html`'s idle panel, which links htmx
already; this shell is a different template and does not. Do not "fix" the
inconsistency by adding the script tag. Do
link `{{ base_path }}/assets/lc_motion.js` — Plan A-vis's helper creates the
`#lc-flights` layer this shell will fire flights into from slice 3.

F.1's order is fixed and no screen may reorder it (spec §7.3):

```html
<body class="lc" data-player-id="{{ player_id }}">
  <div class="lc-status">
    <span id="lc-clock"></span>
    <span>ROOM {{ code }}</span>
  </div>

  {{ banner|safe }}

  <nav class="lc-tabs" role="tablist">
    <button type="button" class="lc-tab" data-lc-tab="hand"  role="tab" aria-selected="true">HAND</button>
    <button type="button" class="lc-tab" data-lc-tab="table" role="tab" aria-selected="false">TABLE</button>
    <button type="button" class="lc-tab" data-lc-tab="log"   role="tab" aria-selected="false">LOG</button>
  </nav>

  <main class="lc-view">
    <section class="lc-pane" data-lc-pane="hand">{{ hand_pane|safe }}</section>
    <section class="lc-pane" data-lc-pane="table" hidden>
      <p class="lc-empty">The table lands in the next slice.</p>
    </section>
    <section class="lc-pane" data-lc-pane="log" hidden>
      <p class="lc-empty">Nothing logged yet.</p>
    </section>
  </main>

  <div class="lc-actions">
    <button type="button" class="lc-btn lc-btn-drink" disabled>DRINK</button>
    <button type="button" class="lc-btn lc-btn-secondary" disabled>PASS</button>
  </div>
</body>
```

Three things that are rules, not preferences:

- **Tabs are always HAND / TABLE / LOG in that order, and the active tab is
  never hoisted** — only its colour and 2px underline change (F.1). The
  underline colours are per-tab, bound in Task 2's CSS by `data-lc-tab`.
- **LOG stays in the tab row even though it has no content.** Omitting it would
  break F.1's ordering rule, which is the spec this slice is built to; an empty
  pane does not (spec §7.3).
- **The action bar's buttons are disabled placeholders** — the beat's decision
  is slice 3. The drinking option is the amber one (F.1) and keeps its class so
  slice 3 changes behaviour, not layout.

- [ ] **Step 2: The inline script — tabs and clock only**

At the end of `<body>`, matching `room.html`'s placement. **No `EventSource` in
this task** — the SSE client is Task 3, deliberately, so nobody half-writes it
here. No `DOMContentLoaded` binding: the page is standalone and not hx-boosted
(see Global Constraints).

```html
<script>
const BP = "{{ base_path }}", CODE = "{{ code }}";
document.querySelectorAll("[data-lc-tab]").forEach(b => b.addEventListener("click", () => {
  document.querySelectorAll("[data-lc-tab]").forEach(x =>
    x.setAttribute("aria-selected", String(x === b)));
  document.querySelectorAll("[data-lc-pane]").forEach(p =>
    p.hidden = p.dataset.lcPane !== b.dataset.lcTab);
}));
function lcClock() {
  const d = new Date();
  document.getElementById("lc-clock").textContent =
    d.getHours() + ":" + String(d.getMinutes()).padStart(2, "0");
}
lcClock(); setInterval(lcClock, 20000);
</script>
```

- [ ] **Step 3: `lc_hand_pane` — the §7.8 Hand region, in `lc_render.rs`**

Plan A recorded the contract for this component and left the builder to this
plan: root `#lc-hand`, **requires** `data-seq`, **exposes** `data-count`, motion
anchor `hand`. Satisfy all four.

```
<div id="lc-hand" data-seq="{seq}" data-count="{hand.len()}" data-flight-anchor="hand">
  <section class="lc-setup">
    <h2>Your drink</h2>
    <form method="post" action="{base_path}/room/{code}/lastcall/vessel">
      <select name="deck">…five options, value = Deck::slug()…</select>
      <input name="container" maxlength="24" placeholder="50cl can">
      <button type="submit">REGISTER</button>
    </form>
    <h2>Handicaps</h2>
    …one .lc-setup-row per SetupRow…
  </section>
  …one card_face per card…
</div>
```

Each handicap row is its own form — **any room member may set any player's
handicap and it is public** (spec §2, item 2: there is no host or owner concept
in this crate; `presets.rs` records the model as *"not owner-scoped — it's a
friends app; anyone logged in may edit"*, and DDv1 §11 wants the table setting
handicaps rather than the player, to stop everyone declaring themselves a
lightweight). So do **not** gate the row on `player_id == me`; `me` is used only
to append `" (you)"` to your own name.

```
<form class="lc-setup-row" method="post" action="{base_path}/room/{code}/lastcall/handicap">
  <input type="hidden" name="target" value="{player_id}">
  <span>{name}{ (you) }</span>
  <span class="lc-setup-decks">{one card_dot per registered deck}</span>
  <input type="number" name="handicap_pct" min="25" max="300" step="5" value="{handicap_pct}">
  <button type="submit">SET</button>
</form>
```

Plain `method="post"` forms, not HTMX: the handlers redirect back to
`/room/{code}/lastcall`, which is the simplest thing that works for an
undesigned setup form and needs no swap target. When the hand is empty, render
`<p class="lc-empty">Register your drink to be dealt a hand.</p>` in place of
the cards. Cards come from Plan A's `card_face` **unchanged** — this builder
owns the container, not the card, and the container is the throwaway half:
slice 2's HandWheel replaces it and keeps the CardFace.

Unit tests in `lc_render.rs`:

- `test_lc_hand_pane_satisfies_the_contract` — output contains `id="lc-hand"`,
  `data-seq="{seq}"`, `data-count="{n}"` and `data-flight-anchor="hand"`.
- `test_lc_hand_pane_posts_to_prefixed_urls` — with `base_path = "/drinks"`,
  `code = "QK4M"`: contains `action="/drinks/room/QK4M/lastcall/vessel"` and
  `action="/drinks/room/QK4M/lastcall/handicap"`, and one
  `<input type="hidden" name="target" value="…">` per `SetupRow`.
- `test_lc_hand_pane_handicap_rows_are_not_self_gated` — three `SetupRow`s with
  `me = 2`: all three render a `SET` button (only row 2 carries `(you)`). The
  regression this guards is gating the control on ownership, which spec §2
  explicitly rejects.
- `test_lc_hand_pane_empty_hand` — empty `hand` renders `lc-empty`, no
  `lc-cardface`, and `data-count="0"`.

- [ ] **Step 4: `lc_page`**

`load_lc` → build the `SetupRow`s → render. The rows come from the state itself,
not a second `room_members` query — `LastCallState.players` already carries name,
handicap and vessels, and using it keeps the shell and the hand fragment reading
one source:

```rust
fn setup_rows(st: &LastCallState) -> Vec<lc_render::SetupRow> {
    st.players.iter().map(|p| lc_render::SetupRow {
        player_id: p.player_id,
        name: p.name.clone(),
        handicap_pct: p.handicap_pct,
        decks: p.vessels.iter().map(|v| v.deck).collect(),
    }).collect()
}
```

The viewer's own hand is `st.players[st.seat_of(player.id)?].hand`. A logged-in
member who is somehow not seated (a race with Task 1's late-join hook) gets an
empty hand rather than an error.

`load_lc` already gates member → active game → kind, so a non-member gets `403`
and a Ring of Fire room gets `WrongGameKind` for free.

- [ ] **Step 5: `lc_hand_handler` — the private route**

The whole handler is the signature plus five lines. It must not gain a `Form`,
a `Query`, or a second `Path` segment — that is the constraint. Returns
`Html(lc_render::lc_hand_pane(&state.base_path, &code, player.id, hand, &rows,
st.seq))`.

Register both routes in `router()`, next to the Task 1 group:

```rust
.route("/room/{code}/lastcall", get(crate::lc_routes::lc_page))
.route("/room/{code}/lastcall/hand", get(crate::lc_routes::lc_hand_handler))
```

Order matters against axum 0.8's matcher only if a wildcard were involved; these
are literal segments, so `/room/{code}/lastcall/hand` and
`/room/{code}/lastcall` coexist.

- [ ] **Step 6: Tests in `tests/http.rs`**

1. `test_lastcall_shell_renders_fixed_tab_order` — the shell body contains
   `data-lc-tab="hand"`, `"table"`, `"log"` **in that source order** (assert via
   `find()` index comparison, not just `contains`), links
   `/assets/lastcall.css`, and does **not** link `game.css`.
2. `test_lastcall_shell_requires_membership` — a logged-in non-member gets
   `403`; an unauthenticated request gets the `PlayerSession` redirect/rejection
   the crate already produces for `/room/{code}` (assert the same status the
   existing room-page test asserts).
3. `test_lastcall_hand_is_private` — **the one that matters** (spec §8). alice
   and bob both register different decks (`beer` and `wine`). Then:
   - alice's `GET /room/{code}/lastcall/hand` contains `beer-01` and **not**
     `wine-01`;
   - bob's contains `wine-01` and **not** `beer-01`;
   - the request with **no cookie** does not return `200`.
   A live two-session test, alongside the structural guarantee that there is no
   input naming a player.
4. `test_lastcall_hand_route_takes_no_player_input` — appending
   `?player_id={bob}` or `?target={bob}` to alice's request changes nothing: the
   response is byte-identical to the un-parameterised one. This asserts the §6.1
   constraint behaviourally as well as by signature.
5. `test_lastcall_hand_rejects_wrong_game_kind` — with Ring of Fire active,
   `GET /room/{code}/lastcall/hand` is `409` "belongs to the other game".
6. `test_lastcall_shell_shows_all_handicap_rows` — alice's shell contains a
   handicap form for bob as well as for herself (the spec §2 rule again, at the
   page level).

- [ ] **Step 7: Commit**

```bash
git add drinkinggame/templates/lc_room.html drinkinggame/src/lc_routes.rs \
        drinkinggame/src/lc_render.rs drinkinggame/src/routes.rs drinkinggame/tests/http.rs
git commit -m "feat(drinks): Last Call phone shell and the private hand fragment route"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

> ### Browser checkpoint 1 — after Task 2
>
> `cargo run -p drinkinggame` (serves standalone on `:3001`, no portfolio, no
> nginx). Two browser profiles so there are two sessions.
>
> 1. Log in as two players, create a room in one, open the room URL in the
>    other — both are members.
> 2. Start **Last Call** from the idle panel's third start card. Both phones,
>    on their next load of `/room/{CODE}`, land on `/room/{CODE}/lastcall`.
> 3. The shell shows: status row with clock and `ROOM {CODE}`, the phase banner
>    reading `DRAW` in amber with `ROUND 1 · BEAT 1 OF 6`, tabs HAND / TABLE /
>    LOG in that order with the violet HAND underline, and the disabled amber
>    action bar. Tapping TABLE and LOG switches panes and moves the underline
>    without reordering the tabs.
> 4. Register `beer` in one browser and `wine` in the other. Each phone's HAND
>    tab shows **four** CardFaces in its own deck colour, and **neither shows the
>    other's cards**. Confirm in devtools that the other player's card ids are
>    not present anywhere in the DOM.
> 5. From one browser, set the *other* player's handicap to 150 — it sticks.
> 6. Open a Ring of Fire room in a third tab and confirm `/room/{CODE}` still
>    renders the old shell with no redirect.
>
> Fixture-only cases (seven seats, two-deck plaques, oversized hands) are
> asserted in unit tests, not demonstrated here — spec §8 puts them out of the
> browser acceptance criteria for this slice.

---

### Task 3: The SSE contract and the client-side stale-drop rule

**Class:** C (logic tests cannot encode — reviewer required)

**Why this class:** broadcast ordering. `RoomHub` is a per-room broadcast whose
subscribers include the *unauthenticated* spectator screen, so what goes into
`LcPublic` is a privacy boundary, not a rendering choice. And the race the
stale-drop rule exists for — a slow private fetch landing after a newer tick and
repainting an older hand — is a wall-clock interleaving between an SSE stream
and an independent XHR that no unit test reproduces. A reviewer checks that the
publishes happen while the room lock is still held (the `1e742d4` bug), that the
snapshot frame is emitted only for Last Call rooms, and that the client's
"highest seq wins" rule is applied to every path that repaints.

**Files:**
- Modify: `drinkinggame/src/hub.rs` (two `RoomMessage` variants + test)
- Modify: `drinkinggame/src/routes.rs` (`sse_stream` — snapshot frame and two
  forwarding arms)
- Modify: `drinkinggame/src/lc_routes.rs` (`persist_and_broadcast_lc` body)
- Modify: `drinkinggame/templates/lc_room.html` (the `EventSource` client)
- Test: `drinkinggame/src/hub.rs`, `drinkinggame/tests/http.rs`

**Interfaces:**
- Consumes: `lc_render::lc_public_panel(&PublicView) -> String` and
  `LastCallState::public_view()` (**Plan A**); `persist_and_broadcast_lc`
  (Task 1).
- Produces:

```rust
// hub.rs — appended to RoomMessage, above `Ended`
/// Rendered PUBLIC Last Call fragment (phase banner now; felt, plaques,
/// hand sizes and deck counts in Plan B). Broadcast to everyone including
/// the unauthenticated spectator screen — rendered from `PublicView`, so it
/// cannot contain unrevealed card identity by construction (spec §3.4).
LcPublic(String),
/// The game's current `seq`. Carries no state — it only tells each phone to
/// re-fetch its own private fragment.
LcTick(u64),

// lc_routes.rs
pub(crate) async fn broadcast_lc(state: &GameState, room_id: i64, st: &LastCallState);
```

SSE event names: `lcpublic` and `lctick`.

- [ ] **Step 1: The hub variants**

Add both to `RoomMessage`. Extend `test_subscribe_publish_remove` with an
`LcPublic` and an `LcTick` case in the existing style (publish, `recv`, match,
`assert_eq!`).

- [ ] **Step 2: `broadcast_lc` and the publish order**

```rust
/// Publishes the public fragment and then the tick. Both make every phone
/// re-fetch its own hand; the client coalesces the pair into one fetch, and
/// the stale-drop rule makes a duplicate harmless. Two messages rather than
/// one because the spectator screen consumes only `LcPublic` and later
/// private-only transitions (arming a card) will publish only `LcTick`.
pub(crate) async fn broadcast_lc(state: &GameState, room_id: i64, st: &LastCallState) {
    let view = st.public_view();
    state.hub.publish(room_id, RoomMessage::LcPublic(lc_render::lc_public_panel(&view)));
    state.hub.publish(room_id, RoomMessage::LcTick(view.seq));
}
```

Call it from `persist_and_broadcast_lc` (**after** `set_game_state`, so a phone
that fetches on the tick reads the persisted state, and **while the caller still
holds the room lock** — every caller already takes the guard around the whole
body, and releasing before the broadcast would let a concurrent handler's
broadcast land first and leave this request's stale render as the last word;
this is the `1e742d4` bug the crate already learned once):

```rust
crate::game::broadcast_room(state, ctx.room.id, &ctx.room.code).await;
broadcast_lc(state, ctx.room.id, &ctx.st).await;
```

And from `room_page`'s late-join block (Task 1, Step 4 left a marked TODO
there), inside the same guard, only when the state actually changed.

- [ ] **Step 3: `sse_stream`**

Two forwarding arms in the `BroadcastStream` match, alongside the existing five:

```rust
Ok(RoomMessage::LcPublic(html)) => Some(Ok(Event::default().event("lcpublic").data(html))),
Ok(RoomMessage::LcTick(seq)) => Some(Ok(Event::default().event("lctick").data(seq.to_string()))),
```

`LcTick`'s data is `seq.to_string()` and never empty — a named SSE event with an
empty data buffer is silently dropped by the browser's EventSource parser
(WHATWG SSE spec), the same pitfall the two `Ended` branches already carry
comments about. `seq` is a `u64` so it is never empty; say so in a comment
rather than leaving the next reader to rediscover it.

The snapshot: the existing four frames (`leaderboard`, `game`, `screen`, `room`)
are emitted unconditionally today and **existing tests drain exactly four**.
Append a fifth `lcpublic` frame **only** when the room's active game is
`last_call`:

```rust
let lc_initial = match db::get_active_game(&state.pool, room.id).await {
    Some(game) if game.kind == "last_call" => {
        let st = crate::last_call::LastCallState::from_json(
            game.state_json.as_deref().unwrap_or_default());
        Some(crate::lc_render::lc_public_panel(&st.public_view()))
    }
    _ => None,
};
```

chained onto the existing snapshot so Ring of Fire and 3 Man rooms emit exactly
the same four frames they do today. The existing snapshot is
`futures::stream::iter([...])`, and an `Option`'s iterator is **not** a
`Stream` — it has to be wrapped:

```rust
.chain(futures::stream::iter(lc_initial.into_iter().map(|html| {
    Ok::<_, Infallible>(Event::default().event("lcpublic").data(html))
})))
```

placed **before** the existing `.chain(BroadcastStream::new(rx)…)`.

- [ ] **Step 4: The client — coalesced fetch and the stale-drop rule**

Append to `lc_room.html`'s inline script. The rule (spec §5): *SSE ticks and the
phone's own fetch race; a slow fetch can land after a newer tick and repaint an
older hand. The client keeps the highest `seq` it has seen and discards any
fetch response carrying a lower one.*

```html
<script>
// The server-rendered pane is the seq floor; every later repaint must be at
// least this fresh.
let lcSeq = Number(document.getElementById("lc-hand")?.dataset.seq || 0);
let lcPending = null;

function lcApply(html) {
  const tpl = document.createElement("div");
  tpl.innerHTML = html;
  const frag = tpl.querySelector("#lc-hand");
  // Stale-drop: a response older than the newest seq we've seen would
  // repaint an out-of-date hand. Equal seq is fine (a duplicate repaint).
  const seq = Number(frag?.dataset.seq || 0);
  if (!frag || seq < lcSeq) return;
  lcSeq = seq;
  document.querySelector('[data-lc-pane="hand"]').innerHTML = html;
}

// Coalesce: LcPublic and LcTick arrive as a pair for the same change, so a
// naive fetch-per-message doubles every request for no new information.
function lcFetchHand() {
  if (lcPending) return;
  lcPending = setTimeout(() => {
    lcPending = null;
    fetch(BP + "/room/" + CODE + "/lastcall/hand", { credentials: "same-origin" })
      .then(r => r.ok ? r.text() : null)
      .then(html => { if (html !== null) lcApply(html); })
      .catch(() => {});
  }, 60);
}

const es = new EventSource(BP + "/room/" + CODE + "/sse");
es.addEventListener("lcpublic", e => {
  const tpl = document.createElement("div");
  tpl.innerHTML = e.data;
  const root = tpl.querySelector("[data-lc-public]");
  if (root) lcSeq = Math.max(lcSeq, Number(root.dataset.seq || 0));
  const banner = tpl.querySelector("template[data-lc-banner]");
  if (banner) document.getElementById("lc-banner").outerHTML = banner.innerHTML;
  lcFetchHand();
});
es.addEventListener("lctick", e => {
  lcSeq = Math.max(lcSeq, Number(e.data || 0));
  lcFetchHand();
});
es.addEventListener("ended", () => { es.close(); window.location = BP + "/"; });
</script>
```

Two details that are not derivable:

- `lcSeq` is raised **before** the fetch is issued (from the tick's own payload
  and from `data-lc-public`'s attribute), which is what makes an in-flight
  older response droppable when it lands.
- `outerHTML` on the banner, not `innerHTML` — `lc_banner` returns the whole
  `<div class="lc-banner lc-beat-…" id="lc-banner">`, so the hue class travels
  with it. Replacing `innerHTML` would leave the previous beat's hue in place.

This is still a standalone, non-hx-boosted page, so there is no
`DOMContentLoaded`/`htmx:afterSwap` binding and no double-injection guard —
matching `room.html`. That carve-out is recorded in Global Constraints; do not
add them.

- [ ] **Step 5: Tests**

In `hub.rs`: the two variants in `test_subscribe_publish_remove` (Step 1).

In `tests/http.rs`, following `test_tm_end_broadcasts_summary_and_idle`'s
pattern (`into_body().into_data_stream()`, `next().await`):

1. `test_lastcall_sse_snapshot_includes_lcpublic` — with a Last Call game
   active, drain frames and assert one carries `event: lcpublic` with a body
   containing `data-lc-public` and `data-seq=`.
2. `test_rof_sse_snapshot_has_no_lcpublic` — with a **Ring of Fire** game
   active, drain **four** frames, assert none of them contains `lcpublic`, and
   assert the fourth is the `room` frame. Do **not** assert "exactly four" by
   awaiting a fifth `next()` — no fifth frame is ever sent, so that awaits until
   the harness times out. Draining four and checking their contents catches the
   regression a misplaced `.chain` causes (it would land `lcpublic` inside the
   first four) without asserting absence by waiting.
3. `test_lastcall_vessel_broadcasts_public_and_tick` — subscribe to the SSE
   stream, drain the snapshot, then `POST /lastcall/vessel` from another
   session. `persist_and_broadcast_lc` fires `broadcast_room` *before*
   `broadcast_lc`, so the very next frame is `room`, not `lcpublic` — **filter**
   the incoming frames to the `lcpublic`/`lctick` ones rather than asserting
   positionally. Assert `lcpublic` arrives before `lctick`, and that the
   `lctick` payload parses as a `u64` greater than the snapshot's `data-seq`.
4. `test_lcpublic_never_carries_hand_cards` — the privacy assertion at the
   transport layer. Both players register decks; every `lcpublic` frame seen on
   the stream contains none of `beer-01`, `cider-01`, `wine-01`, `liquor-01`,
   `soft-01`.
5. `test_lctick_payload_is_never_empty` — assert the forwarded `lctick` frame's
   data is non-empty (the EventSource empty-buffer pitfall).

- [ ] **Step 6: Commit**

```bash
git add drinkinggame/src/hub.rs drinkinggame/src/routes.rs drinkinggame/src/lc_routes.rs \
        drinkinggame/templates/lc_room.html drinkinggame/tests/http.rs
git commit -m "feat(drinks): Last Call SSE contract with signal-and-fetch and stale-drop"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

> ### Browser checkpoint 2 — after Task 3, before the final review
>
> Same two-session setup as checkpoint 1.
>
> 1. With both phones sitting on the Last Call shell, register a drink in
>    browser A. Browser B's hand pane repaints **without a reload** — and shows
>    B's own cards, not A's.
> 2. Set A's handicap from B. Both phones repaint; the value is the same on
>    both.
> 3. In devtools' Network tab, confirm exactly **one**
>    `GET /room/{CODE}/lastcall/hand` per change, not two — the coalescing works.
> 4. In the Elements tab of the *spectator*-facing SSE payload (or via
>    `curl -N http://localhost:3001/room/{CODE}/sse`), confirm no `lcpublic`
>    frame ever contains a card id.
> 5. Throttle the network to Slow 3G in browser A, then fire three changes from
>    browser B in quick succession. The hand pane must settle on the **newest**
>    state — never flick back to an older one. This is the stale-drop rule.
> 6. Open a Ring of Fire room and a 3 Man room; both behave exactly as before.
>
> Then run the plan-end whole-diff review on the most capable model. Every task
> in this plan already had its own Class C reviewer; this final pass is for the
> seams between them — the lock held across all three broadcast paths, and the
> fact that a Ring of Fire or 3 Man room behaves exactly as it did before.

---

## Before this plan is done

- Every task carries a class and a real acceptance command. **All three are
  Class C** and each gets a task reviewer on a capable model, exactly as spec
  §10 names them: the entry redirect and the cross-game arms, the private hand
  route, and the SSE contract.
- No migration was written, and `cargo sqlx prepare` was not run — neither is
  needed (see Global Constraints).
- Nothing in this plan re-authored a Plan A component or a Plan A-vis
  animation. The only markup added is the §7.8 Hand region; everything else is
  Plan A's builders called as-is. A selector this plan wanted and could not find
  is a bug report against Plan A, and a token that looked wrong on screen should
  have been caught on Plan A-vis's gallery.
- Spec §2's "In" list maps as: (1) Task 1 · (2) Tasks 1+2 · (5) Task 1 ·
  (6) Task 3 · (8) Task 2 — and (3), (4) and part of (7) were **Plan A**, the
  rest of (7) plus §7.7 was **Plan A-vis**, and (9), (10) are **Plan B**.
- `GET /room/{code}/lastcall/table` and the `/room/{code}/screen` kind branch
  are deferred to Plan B by design, recorded in the Routes table so the "every
  spec requirement maps to a task" check does not silently drop them.
- Ring of Fire and 3 Man are provably untouched: `test_room_page_unchanged_for_rof_and_three_man`
  and `test_rof_sse_snapshot_has_no_lcpublic` are the two that say so.
