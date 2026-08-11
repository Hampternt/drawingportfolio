# Last Call — Plan E: the loop wiring (slice 3b)

> **For agentic workers:** REQUIRED SUB-SKILLS: `plan-economics` (this repo's
> task classes, review policy and plan sizing) then
> superpowers:subagent-driven-development to execute task-by-task.

**Goal:** Wire Plan D's engine to the room — POST action routes, the beat clock
on the existing 1 Hz ticker, the F.1 action bar, and the reveal/resolve
presentation — so a Last Call game is playable end to end with the placeholder
catalog.

**Architecture:** Routes own everything the engine refuses to: RNG (drawn cards
are sampled in the draw route, the `three_man.rs` dice pattern), the clock (a
persisted `beat_deadline_ms` field written only by routes and the ticker — the
engine never reads a clock), and error→status mapping. Every mutating route
takes the room's `RoomLocks` guard in the exact `tm_routes.rs` shape and calls
Plan D's transitions verbatim. The SSE contract is unchanged: no new event, no
new publish order; private actions ride `LcTick` alone, public ones ride the
existing `persist_and_broadcast_lc`. New client behaviour lives in one
`node --check`-gated asset, `lc_loop.js` — the inline shell scripts gain only
guarded one-line hooks.

**Slice:** When this plan is done, phones drive a full round: START ROUND 1
out of the registration lobby, finish-&-draw at later Draw beats, arm/disarm
via Plan C's wheel events, target and LOCK IN, the timer advancing every timed
beat, the reveal flipping plays onto the big-screen felt with card flights, HP
moving, elimination showing, and a frozen game-over tableau ending through the
existing `lc_end_handler` idle-panel handoff. **The minimal game-over summary
(banner + centre victory line + END GAME button) ships here; the designed
end-of-game screen is Plan J. Real cards and the damage scale are Plan F.**

**Ledger:** `.superpowers/sdd/2026-08-11-last-call-plan-e-loop-wiring/progress.md`
(gitignored).

---

## Proposed design decisions — awaiting user review

The bundle is silent on each of these; the plan proposes rather than stalls.

1. **E1 — Round 1's Draw beat is the untimed registration lobby.** A 30s timer
   there would advance past `set_vessel`'s Draw gate (D15) before anyone
   registered a drink. Any member presses **START ROUND 1** (a new `begin`
   route, `tm_roll`'s any-member precedent) once ≥ 2 players have vessels;
   Draw beats of round ≥ 2 run the normal 30s clock.
2. **E2 — Timer state is a persisted engine *field*, not engine behaviour:**
   `LastCallState.beat_deadline_ms: Option<i64>` (unix ms), written only by
   `lc_routes::arm_beat_clock`/the ticker with a caller-supplied `now`. Data,
   not a clock — the engine stays pure, the deadline survives a restart, and
   the spectator snapshot can render the timer.
3. **E3 — Early beat exits: only "all locked" is implemented** (the one exit
   the engine can see). Draw's "all answered" and Diplomacy's "all ready" need
   ready-tracking no engine field carries — timer-only this slice, recorded
   here rather than half-built. Reveal always runs its 20s as the
   look-at-the-flip pause (no reaction system exists to fill it).
4. **E4 — Auto beats collapse in one locked pass:** a due advance runs
   `advance_beat`/`resolve` and then chains through Deal and Resolve
   (`duration_secs() == None`) until it lands on a timed beat or a game-over
   freeze — one lock acquisition, one broadcast at the end of the chain.
5. **E5 — arm/disarm/set_target publish `LcTick` only** (new helper
   `persist_and_tick_lc`); begin/draw/lock and every ticker advance publish
   the full `persist_and_broadcast_lc` set. Rationale under Task 1.
6. **E6 — `hand_len` projects `hand + armed + own staged plays` until the
   reveal.** Otherwise arming visibly shrinks a public number and leaks the
   armed count DDv2 §6.3 says stays secret until the lock tick. After the
   reveal the extra terms are structurally zero and the count is honest again.
7. **E7 — Action-bar copy** is the exact-string table in Task 4 (START ROUND 1
   / FINISH {DECK} · DRAW / LOCK IN / DRINK {n} / END GAME, plus the hint
   lines). Every drinking-adjacent primary carries `lc-btn-drink` (amber, F.1).
8. **E8 — The target picker is a per-card seat `<select>` section in the hand
   pane** (Lock beat, unlocked, `targets == "one"` cards only), fetch-POSTing
   on change — not folded into Plan C's `armed_column`, which stays untouched.
9. **E9 — The reveal's pay display is a static amber chip "DRINK {n}"**, n =
   the viewer's own charged pulls (sum of `pull_cost` over their plays) — the
   engine debits vessels; the human is told what to actually drink.
10. **E10 — The beat timer becomes a child of `#lc-banner`** and is
    deadline-driven client-side: `beat_timer_live` emits `data-duration-ms` +
    `data-deadline-ms`, `lc_loop.js` computes remaining and sets
    `--lc-beat-ms`. Amends the §7.8 BeatTimer contract (`data-elapsed-ms` →
    `data-deadline-ms`); clock skew and a mid-beat joiner's slightly-fast rail
    are accepted for a party game.
11. **E11 — The flight layer moves into the static shells** (`lc_room.html`,
    `lc_screen.html`, one `#lc-flights` each); `lc_screen_panel` and
    `lc_mini_table` stop emitting it. Closes the STATUS flight-layer debt
    structurally: no repaint can destroy a mid-flight layer again.
12. **E12 — The SSE lag arm emits a synthetic `lctick` with data `"0"`**
    instead of dropping the frame: `"0"` never lowers the client's seq floor
    (`Math.max`), but the listener still re-fetches, and stale-drop makes the
    re-fetch safe. RoF/3 Man clients have no `lctick` listener — no-op there.
13. **E13 — Game over = the frozen Resolve tableau** (D16) + banner "GAME
    OVER", a centre victory line on the felt, and an amber END GAME button in
    the thumb zone posting the existing `/lastcall/end`. The designed
    end-of-game screen is Plan J.
14. **E14 — All new phone/screen JS lives in `assets/lc_loop.js`** (served
    like `lc_motion.js`, `node --check`-gated). The inline shell scripts gain
    only guarded one-liners (`if (window.lcLoop…)`) — the STATUS-recorded
    silent-failure property of inline `<script>` is not extended with logic.
15. **E15 — Reveal presentation: the big-screen felt centre shows revealed
    plays** as CardMinis with a "{SRC} → {TGT}" micro-caption in `order_key`
    order (the centre was recorded empty by Plan B; this plan owns filling
    it). The phone's reveal is the banner + DRINK chip; the mini-table centre
    stays the decorative pile. Discard flights are deferred with the discard
    presentation.
16. **E16 — The ticker DB-polls each hub-active room once per second** (one
    indexed SELECT) rather than keeping an in-memory deadline registry —
    self-healing across restarts, trivially bounded, and only rooms with live
    subscribers are polled at all.

---

## Global Constraints

Every task's requirements implicitly include this section.

**Spec bindings carried from slices 1–3a — all still in force:**

- `GET /room/{code}/lastcall/hand` keeps taking **no player identifier**; no
  new private route takes one either. Mutating routes name no player — the
  actor is the session cookie (`PlayerSession`), targets are *seats* validated
  by the engine.
- Publish order `room` → `lcpublic` → `lctick` (inside
  `persist_and_broadcast_lc`: game, room, lcpublic, lctick) is unchanged.
  `broadcast_lc` stays await-free under the room guard — every builder it
  calls remains a pure string build with no suspension point (`1e742d4`).
- **No new SSE event.** `LcPublic`/`LcTick` carry everything this plan ships;
  the one transport change (the lag arm, Task 3) reuses `lctick`.
- Tests that assert on SSE **filter frames by event name / content via
  `read_sse_until`, never index positionally**.
- Public renderers take `&PublicView`; the new `ActionBarView` is a
  private-side view (per-viewer, never broadcast). Renderers emit deck class
  names, never hex — new builders join the `no_hex` sweep.
- Spec §3.4.1 is enforced by Plan D (`locked_plays`); this plan must not
  bypass it — no route touches `plays` directly, ever.
- Ring of Fire and 3 Man are untouched. Named pins: `test_rof_sse_snapshot_has_no_lcpublic`,
  `test_tm_sse_snapshot_includes_tm_panels`, `test_sse_snapshot_has_all_stateful_kinds`
  must stay green after every task; any route/ticker change names them in its
  report.
- One `@media (prefers-reduced-motion: reduce)` block in `lastcall.css`;
  additions go inside the existing block.

**Consumed from Plan D (exact — build against these, not today's stubs):**

```rust
pub fn arm(&mut self, player_id: i64, card_id: &str) -> Result<(), LcError>;
pub fn disarm(&mut self, player_id: i64, card_id: &str) -> Result<(), LcError>;
pub fn set_target(&mut self, player_id: i64, card_id: &str, target: Option<usize>) -> Result<(), LcError>;
pub fn lock_in(&mut self, player_id: i64) -> Result<(), LcError>;
pub fn advance_beat(&mut self) -> Result<(), LcError>;
pub fn resolve(&mut self) -> Result<(), LcError>;
pub fn finish_and_draw(&mut self, player_id: i64, vessel_idx: usize, drawn: Vec<Card>) -> Result<(), LcError>;
pub fn outcome(&self) -> Option<LcOutcome>;          // + PublicView.outcome
impl Beat { pub fn duration_secs(self) -> Option<u16>; } // Draw 30 / Deal None / Diplomacy 60 / Lock 45 / Reveal 20 / Resolve None
pub enum LcError { NotSeated, BadHandicap, WrongBeat, NotAlive, AlreadyLocked,
    UnknownCard, NotPlayable, CantAfford(String), NeedsTarget(String),
    BadTarget, BadDraw, MustResolve }
pub struct ArmedCard { pub card: Card, pub target: Option<usize> } // LcPlayer.armed: Vec<ArmedCard>
// LastCallState.locked_plays: Vec<Play> — public_view() never reads it.
```

**Consumed from Plan C (exact):** `lc:arm`/`lc:disarm` bubbling CustomEvents
(detail `{ cardId }`) that nothing yet listens to — this plan attaches **one
delegated listener each on the shell body and POSTs the intent; never rebound
per repaint** (Plan C Task 4's stated contract); `window.lcWheelInit(root?)`;
the single `armed` flight anchor inside `#lc-hand`; the shared
`hand_pane_html(base_path, code, st, player_id) -> String` in `lc_routes.rs`.

**The C/D seam this plan inherits:** Plan C's `HandGroupView.armed` is
`&[Card]` while Plan D's D17 makes `LcPlayer.armed` `Vec<ArmedCard>`. Whichever
plan executed second already reconciled `hand_pane_html` (the expected shape:
`let armed_cards: Vec<Card> = p.armed.iter().map(|a| a.card.clone()).collect();`
feeding `HandGroupView`). Task 4 builds on whatever reconciliation is in the
tree; if none exists, Task 4 adds exactly that mapping and notes it in its
report.

**Baseline:** `./scripts/verify.sh` green after Plans C and D — tests above
the 371-count baseline (quote the actual number in Task 1's report and grow
from there), **17 distinct** clippy warnings, all in `drawingportfolio`;
`drinkinggame` is clean and must stay clean. No migration, no
`cargo sqlx prepare` (`drinkinggame` is runtime-checked).

**Verification for every task:** `./scripts/verify.sh` — all green, output
quoted in the report. Never bare `cargo test`.

**Browser checkpoints:** after Task 4 (the loop is first drivable by hand) and
after Task 5, before the final review. Not per task.

---

### Task 1: The action routes — arm, disarm, target, lock, draw

**Class:** C (auth-gated mutations, per-room locking, and a broadcast-policy
decision — which frames each action publishes — that no unit test can decide)

**Why this class:** every handler holds the room guard across
load→transition→persist→publish; the tick-only vs full-publish split and the
`hand_len` secrecy projection are cross-surface invariants a reviewer must
check against "who is subscribed and what are they looking at".

**Files:**
- Modify: `drinkinggame/src/lc_routes.rs` (new handlers + `map_lc` +
  `persist_and_tick_lc`, below the existing handlers)
- Modify: `drinkinggame/src/routes.rs` (route registration, beside the
  existing `/lastcall/end` line ~888)
- Modify: `drinkinggame/src/last_call.rs` (`public_view()`'s `hand_len`
  projection + one unit test)
- Test: `drinkinggame/tests/http.rs`

**Interfaces:**
- Consumes: Plan D's transitions (Global Constraints block, verbatim);
  `lc_lock`, `load_lc`, `LcCtx`, `persist_and_broadcast_lc` (`lc_routes.rs`);
  `GameError::{NotYourCall, OutOfTurn}` (403 / 409 per `error.rs`);
  `lc_cards::deck_cards`; `DRAW_PER_VESSEL`.
- Produces (Tasks 2 and 4 build against these — exact):

```rust
// lc_routes.rs
#[derive(Deserialize)] pub struct CardForm { pub card_id: String }
#[derive(Deserialize)] pub struct LcTargetForm { pub card_id: String, #[serde(default)] pub target: String }
#[derive(Deserialize)] pub struct DrawForm { pub vessel: usize }

pub async fn lc_arm_handler(State, PlayerSession, Path<String>, Form<CardForm>) -> Response;
pub async fn lc_disarm_handler(/* same */) -> Response;
pub async fn lc_target_handler(State, PlayerSession, Path<String>, Form<LcTargetForm>) -> Response;
pub async fn lc_lock_handler(State, PlayerSession, Path<String>) -> Response;
pub async fn lc_draw_handler(State, PlayerSession, Path<String>, Form<DrawForm>) -> Response;

pub(crate) fn map_lc(e: LcError) -> axum::response::Response;
pub(crate) async fn persist_and_tick_lc(state: &GameState, ctx: &LcCtx);
```

Routes (registered in `routes.rs`, matching the existing `lastcall` lines):

| Method | Path | Body | Publishes |
| --- | --- | --- | --- |
| POST | `/room/{code}/lastcall/arm` | `card_id` | tick only |
| POST | `/room/{code}/lastcall/disarm` | `card_id` | tick only |
| POST | `/room/{code}/lastcall/target` | `card_id`, `target` (`""` = none) | tick only |
| POST | `/room/{code}/lastcall/lock` | — | full |
| POST | `/room/{code}/lastcall/draw` | `vessel` | full |

- [ ] **Step 1: `map_lc` and `persist_and_tick_lc`**

```rust
/// Engine error -> HTTP. NotSeated/NotAlive are "you have no say here" (403,
/// like tm's NotYourCall); WrongBeat/AlreadyLocked/MustResolve are "not now"
/// (409, like tm's OutOfTurn); the two named-card refusals carry their
/// message as a plain-text 422 body the action bar shows verbatim (DDv2 6.3
/// "naming the card"); everything else is a bare 422.
pub(crate) fn map_lc(e: LcError) -> axum::response::Response {
    match e {
        LcError::NotSeated | LcError::NotAlive => GameError::NotYourCall.into_response(),
        LcError::WrongBeat | LcError::AlreadyLocked | LcError::MustResolve => {
            GameError::OutOfTurn.into_response()
        }
        LcError::CantAfford(id) => {
            (StatusCode::UNPROCESSABLE_ENTITY, format!("Can't afford {id}.")).into_response()
        }
        LcError::NeedsTarget(id) => {
            (StatusCode::UNPROCESSABLE_ENTITY, format!("{id} needs a target.")).into_response()
        }
        _ => StatusCode::UNPROCESSABLE_ENTITY.into_response(),
    }
}

/// The private-action twin of `persist_and_broadcast_lc`: persist, then
/// publish ONLY `LcTick`. arm/disarm/set_target change nothing any public
/// surface renders (decision E6 keeps even the public hand size still), so
/// the game/room/lcpublic frames would carry no information — but every
/// phone still needs the tick to re-fetch its own private fragment, and the
/// actor's own repaint arrives that way. "Who is subscribed and what are
/// they looking at": phones re-fetch, the spectator screen ignores lctick by
/// having no listener for it. Publishes while the caller's guard is held,
/// after set_game_state, with no await between render and publish — the
/// same discipline as broadcast_lc.
pub(crate) async fn persist_and_tick_lc(state: &GameState, ctx: &LcCtx) {
    db::set_game_state(&state.pool, ctx.game.id, &ctx.st.to_json()).await;
    db::touch_room(&state.pool, ctx.room.id).await;
    state
        .hub
        .publish(ctx.room.id, crate::hub::RoomMessage::LcTick(ctx.st.seq));
}
```

- [ ] **Step 2: the five handlers.** All five follow one shape — `lc_arm_handler`
verbatim, the rest are the same skeleton around a different transition
(`tm_roll_handler` is the crate precedent):

```rust
pub async fn lc_arm_handler(
    State(state): State<GameState>,
    PlayerSession(player): PlayerSession,
    Path(code): Path<String>,
    Form(form): Form<CardForm>,
) -> axum::response::Response {
    let lock = match lc_lock(&state, &code).await {
        Ok(l) => l,
        Err(r) => return r,
    };
    let _guard = lock.lock().await;
    let mut ctx = match load_lc(&state, &code, &player).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    if let Err(e) = ctx.st.arm(player.id, &form.card_id) {
        return map_lc(e);
    }
    persist_and_tick_lc(&state, &ctx).await;
    StatusCode::NO_CONTENT.into_response()
}
```

Differences only:
- `lc_disarm_handler`: `ctx.st.disarm(...)`.
- `lc_target_handler`: parse first — `form.target.is_empty()` → `None`, else
  `form.target.parse::<usize>()` → `Some(n)` or a bare 422 on parse failure;
  then `ctx.st.set_target(player.id, &form.card_id, target)`.
- `lc_lock_handler`: `ctx.st.lock_in(player.id)`, then
  **`persist_and_broadcast_lc`** (the lock tick is public). Task 2 adds the
  all-locked early advance into this handler.
- `lc_draw_handler`: the one route with RNG. Pre-reads under the guard:
  `seat_of(player.id)` → else `GameError::NotYourCall`; the vessel's deck via
  `ctx.st.players[seat].vessels.get(form.vessel)` → else bare 422; the shoe
  count for that deck from `ctx.st.deck_counts`. Then sample —

```rust
let need = DRAW_PER_VESSEL.min(shoe_count as usize);
let pool_cards = crate::lc_cards::deck_cards(deck);
let drawn: Vec<Card> = {
    let mut rng = rand::thread_rng();
    (0..need)
        .map(|_| pool_cards[rng.gen_range(0..pool_cards.len())].clone())
        .collect()
};
```

  — call `ctx.st.finish_and_draw(player.id, form.vessel, drawn)`, `map_lc` on
  error, then **`persist_and_broadcast_lc`** (deck counts and the drawing
  pulse are public). Card identity is decided here, never in the engine (D6);
  duplicates are fine in the placeholder era.

Register all five in `routes.rs` beside the `/lastcall/end` line:

```rust
.route("/room/{code}/lastcall/arm", post(crate::lc_routes::lc_arm_handler))
.route("/room/{code}/lastcall/disarm", post(crate::lc_routes::lc_disarm_handler))
.route("/room/{code}/lastcall/target", post(crate::lc_routes::lc_target_handler))
.route("/room/{code}/lastcall/lock", post(crate::lc_routes::lc_lock_handler))
.route("/room/{code}/lastcall/draw", post(crate::lc_routes::lc_draw_handler))
```

- [ ] **Step 3: the `hand_len` secrecy projection (decision E6).** In
`public_view()`:

```rust
// DDv2 6.3: before the reveal, only the lock tick is public. hand_len must
// therefore not move while a player stages cards — armed and staged-locked
// cards still count as "in hand" to the room. After the Lock->Reveal flip
// both extra terms are structurally zero (armed cleared, locked_plays
// drained into plays), so the count drops exactly when the plays go public.
hand_len: p.hand.len()
    + p.armed.len()
    + self
        .locked_plays
        .iter()
        .filter(|pl| pl.source_seat == p.seat)
        .count(),
```

Unit test in `last_call.rs` (fixtures from Plan D's tests module):

```rust
#[test]
fn test_public_hand_size_holds_still_while_staging() {
    let mut st = at_lock(); // alice holds 4 beer cards
    assert_eq!(st.public_view().seats[0].hand_len, 4);
    st.arm(1, "beer-01").unwrap();
    assert_eq!(st.public_view().seats[0].hand_len, 4); // armed still counts
    st.set_target(1, "beer-01", Some(1)).unwrap();
    st.lock_in(1).unwrap();
    assert_eq!(st.public_view().seats[0].hand_len, 4); // staged still counts
    st.advance_beat().unwrap(); // the reveal
    assert_eq!(st.public_view().seats[0].hand_len, 3); // now it is public
}
```

- [ ] **Step 4: http tests.** Shared rig helper at the top of the Last Call
test block, following the `set_game_state` pattern (http.rs:2213): log in
alice+bob, create room, `POST /lastcall/start`, then build the state by hand —
`LastCallState::new` with the real player ids, `set_vessel(alice, Beer, "can")`
and `set_vessel(bob, Cider, "bottle")` at Draw, `st.beat = Beat::Lock`,
`set_game_state`. Returns `(app, pool, code, alice, bob, alice_id, bob_id)`.

```rust
#[tokio::test]
async fn test_lc_arm_is_a_tick_only_broadcast() {
    // SSE open, drain the snapshot with read_sse_until(&mut body, "event: lcpublic").
    // POST arm card_id=beer-01 as alice -> 204.
    // let seen = read_sse_until(&mut body, "event: lctick").await;
    // The newly-read segment is ONLY the tick: no lcpublic, no game, no room
    // frame rode along (decision E5):
    // assert!(!seen.contains("event: lcpublic") && !seen.contains("event: game")
    //         && !seen.contains("event: room"), "{seen}");
    // GET hand as alice: contains "ARMED 1" and "beer-01".
    // GET hand as bob: contains "ARMED 0", does NOT contain "beer-01".
}

#[tokio::test]
async fn test_lc_lock_publishes_the_tick_not_the_cards() {
    // alice: POST target card_id=beer-01&target=1 -> 204; POST lock -> 204.
    // read_sse_until "event: lcpublic": the frame carries the seat's locked
    // marker (` data-locked`) and does NOT contain "beer-01" nor the card's
    // title "Nudge" — transport-level proof of §3.4.1 across this route.
}

#[tokio::test]
async fn test_lc_action_routes_are_guarded() {
    // carol (logged in, not a member): POST arm -> 403.
    // A three_man room: POST lock -> 409 (WrongGameKind).
    // No active game: POST draw -> 409/404 per load_lc's NoActiveGame.
}

#[tokio::test]
async fn test_lc_draw_deals_five_from_the_vessels_deck() {
    // Rig round=2, beat=Draw, alice Beer shoe at 36 (set deck_counts), hand 4.
    // POST draw vessel=0 -> 204. GET hand: data-count="9", and the fragment
    // contains no "cider-" id (the sample came from Beer alone).
    // POST draw vessel=0 again -> 422 (TBD-5, one per round -> BadDraw).
    // read_sse_until "event: lcpublic" after the first draw: deck counts
    // moved, so the FULL publish ran (frame contains data-lc-public).
}

#[tokio::test]
async fn test_lc_target_accepts_empty_as_none() {
    // arm beer-03 (targets "self"): POST target card_id=beer-03&target= -> 204.
    // POST target card_id=beer-03&target=abc -> 422.
    // POST target card_id=beer-01&target=1 after arming beer-01 -> 204, and
    // alice's hand fragment now marks that select's option as selected (Task
    // 4 renders it; until then assert 204 + seq bump via a second GET's
    // data-seq strictly greater).
}
```

Also update any existing hand-route test that asserts a 4-card `data-count`
against a state this rig now stages differently — check the block at
http.rs:3867–4033 compiles against the rig unchanged (it should; the rig is
additive).

- [ ] **Step 5: Commit**

```bash
git add drinkinggame/src/lc_routes.rs drinkinggame/src/routes.rs drinkinggame/src/last_call.rs drinkinggame/tests/http.rs
git commit -m "feat(lastcall): action routes — arm/disarm/target/lock/draw under the room guard; hand_len holds still while staging"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

### Task 2: The beat clock — deadline field, advance chain, ticker, begin route

**Class:** C (ticker concurrency against action routes, lock discipline across
an async chain, and the double-check pattern — nothing a unit test can decide
alone)

**Why this class:** the ticker and the action routes race for the same room;
the advisory-read-then-relock-then-recheck sequence and the "broadcast under
the guard" rule are exactly the `1e742d4` class of bug a reviewer exists for.

**Files:**
- Modify: `drinkinggame/src/last_call.rs` (`beat_deadline_ms` field +
  `PublicView.beat_deadline_ms` projection)
- Modify: `drinkinggame/src/lc_routes.rs` (`now_ms`, `arm_beat_clock`,
  `lc_advance_chain`, `lc_tick_room`, `lc_begin_handler`, early advance in
  `lc_lock_handler`; `#[cfg(test)] mod tests` for the pure helpers)
- Modify: `drinkinggame/src/mechanics.rs` (`tick` body)
- Modify: `drinkinggame/src/lc_render.rs` (`beat_timer_live`; `lc_banner`
  gains the timer child; `ring_fixture` gains `beat_deadline_ms: None,`)
- Modify: `drinkinggame/assets/lastcall.css` (banner positioning, 3 lines)
- Modify: `drinkinggame/src/routes.rs` (register `begin`)
- Test: `drinkinggame/tests/http.rs`

**Interfaces:**
- Consumes: Task 1's handlers/helpers; `Beat::duration_secs`; `outcome()`;
  `db::get_room_by_id`; `mechanics::spawn_ticker`'s existing
  `active_rooms()` iteration.
- Produces (exact):

```rust
// last_call.rs — LastCallState gains (container #[serde(default)] keeps old
// blobs loading; DATA ONLY, the engine never reads or writes it):
pub beat_deadline_ms: Option<i64>,
// PublicView gains the same field, projected verbatim.

// lc_routes.rs
pub(crate) fn now_ms() -> i64;                                  // SystemTime since UNIX_EPOCH
pub(crate) fn arm_beat_clock(st: &mut LastCallState, now: i64); // E1/E2
pub(crate) fn lc_advance_chain(st: &mut LastCallState, now: i64); // E4
pub(crate) async fn lc_tick_room(state: &GameState, room_id: i64);
pub async fn lc_begin_handler(State, PlayerSession, Path<String>) -> Response;

// lc_render.rs
pub fn beat_timer_live(duration_ms: u32, deadline_ms: i64) -> String;
```

Route: `POST /room/{code}/lastcall/begin` — registered beside Task 1's lines.

- [ ] **Step 1: field + projection.** Add `beat_deadline_ms` to
`LastCallState` (doc comment: unix ms when the current timed beat expires;
`None` = untimed — the round-1 lobby, auto beats, and the frozen game-over
tableau; written only by `lc_routes`) and to `PublicView`; project it in
`public_view()`. Add `beat_deadline_ms: None,` to `ring_fixture()`'s literal
(the only out-of-engine `PublicView` constructor).

- [ ] **Step 2: the pure helpers** in `lc_routes.rs`:

```rust
pub(crate) fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock before 1970")
        .as_millis() as i64
}

/// Decision E1/E2: round 1's Draw is the untimed registration lobby (a timer
/// there would advance past set_vessel's Draw gate before anyone registered);
/// every other beat takes its DDv2 §5 duration or stays untimed (auto beats).
pub(crate) fn arm_beat_clock(st: &mut LastCallState, now: i64) {
    st.beat_deadline_ms = if st.round == 1 && st.beat == Beat::Draw {
        None
    } else {
        st.beat.duration_secs().map(|s| now + i64::from(s) * 1000)
    };
}

/// One user-visible advance plus every auto beat behind it (decision E4):
/// advance (or resolve, at Resolve), then chain through Deal and Resolve
/// until a timed beat or a game-over freeze, then re-arm the clock. The
/// expects are structural: advance_beat only fails at Resolve, which both
/// branches route to resolve(); resolve only fails off Resolve.
pub(crate) fn lc_advance_chain(st: &mut LastCallState, now: i64) {
    if st.beat == Beat::Resolve {
        st.resolve().expect("resolve() at Beat::Resolve cannot fail");
    } else {
        st.advance_beat().expect("advance_beat() off Resolve cannot fail");
    }
    loop {
        if st.outcome().is_some() {
            st.beat_deadline_ms = None; // frozen final tableau (D16)
            return;
        }
        match st.beat {
            Beat::Deal => st.advance_beat().expect("advance_beat() at Deal cannot fail"),
            Beat::Resolve => st.resolve().expect("resolve() at Beat::Resolve cannot fail"),
            _ => break,
        }
    }
    arm_beat_clock(st, now);
}
```

- [ ] **Step 3: the ticker.** `mechanics::tick` becomes one line —
`crate::lc_routes::lc_tick_room(state, room_id).await;` — with its comment
updated (the extension point is no longer empty). In `lc_routes.rs`:

```rust
/// The Last Call beat clock, ridden on mechanics.rs's global 1 Hz ticker
/// (decision E16). Advisory pre-check WITHOUT the lock first — one indexed
/// SELECT per hub-active room per second, almost always returning early —
/// then, only when a deadline has expired: take the room guard, RE-LOAD and
/// RE-CHECK under it (an action route may have advanced the beat between the
/// advisory read and the lock), run the chain, and persist_and_broadcast_lc
/// while the guard is still held. The re-check is what makes the ticker and
/// the lock route's early advance commute instead of double-advancing.
pub(crate) async fn lc_tick_room(state: &GameState, room_id: i64) {
    let Some(game) = db::get_active_game(&state.pool, room_id).await else {
        return;
    };
    if game.kind != "last_call" {
        return;
    }
    let pre = LastCallState::from_json(game.state_json.as_deref().unwrap_or_default());
    if pre.beat_deadline_ms.is_none_or(|d| now_ms() < d) || pre.outcome().is_some() {
        return;
    }

    let Some(room) = db::get_room_by_id(&state.pool, room_id).await else {
        return;
    };
    let lock = state.locks.for_room(room.id);
    let _guard = lock.lock().await;
    let Some(game) = db::get_active_game(&state.pool, room_id).await else {
        return;
    };
    if game.kind != "last_call" {
        return;
    }
    let mut st = LastCallState::from_json(game.state_json.as_deref().unwrap_or_default());
    if st.beat_deadline_ms.is_none_or(|d| now_ms() < d) || st.outcome().is_some() {
        return;
    }
    lc_advance_chain(&mut st, now_ms());
    let ctx = LcCtx { room, game, st };
    persist_and_broadcast_lc(state, &ctx).await;
}
```

(If the tree's clippy dislikes `is_none_or` on this toolchain, use
`map_or(true, …)` — keep `drinkinggame` clean either way.)

- [ ] **Step 4: `lc_begin_handler` + the all-locked early advance.**
`lc_begin_handler` is Task 1's handler skeleton with this body between load
and persist: refuse unless `ctx.st.round == 1 && ctx.st.beat == Beat::Draw`
(`GameError::OutOfTurn`); refuse unless
`ctx.st.players.iter().filter(|p| !p.vessels.is_empty()).count() >= 2`
(`GameError::TooFewPlayers`); then `lc_advance_chain(&mut ctx.st, now_ms())`
(Draw → Deal auto → Diplomacy, 60s armed) and `persist_and_broadcast_lc`.
Any member may press it — the `tm_roll_handler` any-member precedent, named in
a doc comment.

In `lc_lock_handler`, after a successful `lock_in` and before the publish
(decision E3 — the one engine-visible early exit):

```rust
if ctx.st.beat == Beat::Lock
    && ctx
        .st
        .players
        .iter()
        .filter(|p| p.status == Status::Alive)
        .all(|p| p.locked)
{
    lc_advance_chain(&mut ctx.st, now_ms()); // Lock -> Reveal, 20s armed
}
```

- [ ] **Step 5: the live timer (decision E10).** In `lc_render.rs`:

```rust
/// Live twin of `beat_timer` (which the preview keeps): same root id/class,
/// but deadline-driven — lc_loop.js computes remaining client-side and sets
/// --lc-beat-ms. No inline style, so the no-hex/no-style sweeps still hold.
pub fn beat_timer_live(duration_ms: u32, deadline_ms: i64) -> String {
    format!(
        r#"<div id="lc-beat-timer" class="lc-timer" data-duration-ms="{duration_ms}" data-deadline-ms="{deadline_ms}"></div>"#
    )
}
```

`lc_banner` appends it as its last child when the view has a live deadline:

```rust
let timer = match (view.beat_deadline_ms, view.beat.duration_secs()) {
    (Some(deadline), Some(secs)) => beat_timer_live(u32::from(secs) * 1000, deadline),
    _ => String::new(),
};
```

— inside the existing `#lc-banner` div, so the banner-template `outerHTML`
swap in both shells replaces banner and timer atomically (no orphaned timer,
no inline-script bookkeeping). CSS, in the shell section of `lastcall.css`:

```css
/* Plan E: the beat timer is a child of the banner (decision E10) so the
   lcpublic banner swap replaces both atomically. */
#lc-banner { position: relative; }
#lc-banner .lc-timer { position: absolute; left: 0; right: 0; bottom: 0; }
```

The animation itself is Plan A-vis's existing `.lc-timer` rule; until Task 4's
JS sets `--lc-beat-ms` it runs at the 60s fallback — acceptable between tasks.

- [ ] **Step 6: tests.** Pure-helper unit tests in `lc_routes.rs`'s new
`#[cfg(test)] mod tests` (fixtures built inline, the Plan D `at_lock` shape):

```rust
#[test]
fn test_advance_chain_walks_timed_beats_and_skips_auto_ones() {
    // 3 players with vessels, round bumped to 2 so Draw is timed.
    // From Draw at now=1_000_000: chain -> beat Diplomacy (Deal skipped),
    //   deadline Some(1_000_000 + 60_000).
    // From Diplomacy -> Lock, +45_000. From Lock -> Reveal, +20_000.
    // From Reveal: chain runs advance (->Resolve) then resolve() -> round+1,
    //   beat Draw, deadline +30_000 (round >= 2 Draw IS timed).
}

#[test]
fn test_advance_chain_freezes_on_game_over() {
    // 2 players, bob at hp such that alice's staged play kills him; walk to
    // Reveal, then chain from Reveal: resolve() runs, outcome() is
    // Some(Winner(0)), beat stays Resolve, beat_deadline_ms is None.
}

#[test]
fn test_round_one_draw_is_untimed() {
    // arm_beat_clock on a fresh LastCallState::new(...) -> None (E1);
    // same state with round = 2 -> Some(now + 30_000).
}
```

Integration, in `http.rs` (the ticker is already spawned by
`router_with_pool`, so it runs inside every test app; subscribing the SSE
stream is what puts the room in `active_rooms()`):

```rust
#[tokio::test]
async fn test_the_ticker_advances_an_expired_beat() {
    // Task 1's rig at Beat::Lock, then st.beat_deadline_ms = Some(now_ms - 2_000)
    // before set_game_state. Open SSE. read_sse_until(&mut body,
    // r#"data-beat="reveal""#) — the 1 Hz ticker must deliver the advanced
    // lcpublic frame within read_sse_until's 5s window. Assert the frame also
    // carries data-lc-public (it is the full publish, not a bare tick).
}

#[tokio::test]
async fn test_begin_starts_the_round_and_arms_diplomacy() {
    // Rig at round 1 Draw with both vessels registered (no beat override).
    // POST begin as bob (any member) -> 204; read_sse_until
    // r#"data-beat="diplomacy""# — Deal was chained through; the frame's
    // banner carries data-deadline-ms (the timer is live).
    // Then POST begin again -> 409 (not round-1 Draw any more).
}

#[tokio::test]
async fn test_locking_the_whole_table_advances_early() {
    // Rig at Lock (no deadline needed). alice: target + lock. bob: lock
    // (locking nothing is legal). read_sse_until r#"data-beat="reveal""# —
    // the second lock advanced without any timer. Frame filtered by content,
    // never by position.
}
```

- [ ] **Step 7: Commit**

```bash
git add drinkinggame/src/last_call.rs drinkinggame/src/lc_routes.rs drinkinggame/src/mechanics.rs drinkinggame/src/lc_render.rs drinkinggame/src/routes.rs drinkinggame/assets/lastcall.css drinkinggame/tests/http.rs
git commit -m "feat(lastcall): the beat clock — persisted deadline, auto-beat chain, 1 Hz ticker, begin route, all-locked early advance"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

### Task 3: SSE debts — the lag re-fetch and the flight layer's new home

**Class:** C (an SSE transport-contract change affecting every room kind, and
a cross-surface structural invariant — the layer's containing block and
repaint survival — that tests can only partially encode)

**Why this class:** the lag arm touches the one stream all three games share;
the layer move re-draws a boundary three plans deferred, and "no repaint can
destroy a mid-flight layer" is checkable only as absence+presence plus a
reviewer reading the swap paths.

**Files:**
- Modify: `drinkinggame/src/routes.rs` (the `Err(_) => None` arm, ~line 627)
- Modify: `drinkinggame/src/lc_render.rs` (`lc_screen_panel` ~556,
  `lc_mini_table` ~636 — drop the emitted `#lc-flights`; rewrite the
  `lc_mini_table` doc comment at 571–593 and the sibling-not-nested comment
  at 541–554)
- Modify: `drinkinggame/templates/lc_room.html` (add the layer; replace the
  stale pane comment at lines 26–31)
- Modify: `drinkinggame/templates/lc_screen.html` (add the layer)
- Test: `drinkinggame/src/lc_render.rs` tests, `drinkinggame/tests/http.rs`

**Interfaces:**
- Consumes: `RoomMessage::LcTick`'s existing `lctick` wire event; `ensureLayer`
  in `lc_motion.js` (finds an existing `#lc-flights` before creating one —
  the guard that makes a static layer compatible); `body.lc` / `body.lc-screen`
  `position: relative` (already pinned by Plan A2 / Plan B tests).
- Produces: exactly one static `#lc-flights` per shell page; zero in any
  broadcast or fetched fragment; a lag-proof re-fetch signal.

- [ ] **Step 1: the lag arm (STATUS debt 2, decision E12).** Replace
`Err(_) => None` in the SSE stream match:

```rust
// A lagged receiver dropped RoomMessage frames. Every content variant is a
// complete replacement, so the next one heals — except a dropped LcTick,
// which IS the re-fetch signal (plan-B review finding M5): a phone that
// misses one can stay stale until an unrelated later change. So a lag emits
// a synthetic lctick with data "0": zero never lowers the client's seq
// floor (the listener keeps max(lcSeq, data)), but it still fires the
// coalesced re-fetch, and the stale-drop rule makes that fetch safe
// whatever was missed. Ring of Fire / 3 Man clients register no lctick
// listener, so for them the frame is inert — their four-frame contract is
// about the snapshot, not the live stream (see
// test_rof_sse_snapshot_has_no_lcpublic, which stays green).
Err(_) => Some(Ok(Event::default().event("lctick").data("0"))),
```

- [ ] **Step 2: the layer move (STATUS debt 1 — this plan is the named
owner; decision E11).** In `lc_render.rs`: delete the trailing
`<div id="lc-flights"></div>` from **both** `lc_screen_panel`'s and
`lc_mini_table`'s format strings. Replace `lc_mini_table`'s "the phone's only
flight layer lives in here" doc block and `lc_screen_panel`'s
sibling-not-nested comment with short notes: the layer is static in the shell
templates now, precisely so an `lcpublic`/table repaint can never destroy a
mid-flight node and drop its `onArrive` (`lc_render.rs:570-592` was the debt's
named line).

In `lc_room.html`, immediately before the `<script>` tag (last child of
`body.lc`, whose `position: relative` is its containing block):

```html
<div id="lc-flights"></div>
```

Replace the lines 26–31 comment with: the layer is a static sibling of
`.lc-view` — outside every repainted pane, visible regardless of the active
tab, found by `ensureLayer`'s existing-layer guard.

In `lc_screen.html`, the same `<div id="lc-flights"></div>` immediately after
`#lc-screen-panel` (last child of `body.lc-screen`, `position: relative` per
`lastcall.css:632`).

- [ ] **Step 3: tests.** In `lc_render.rs`: update the tests that currently
assert `id="lc-flights"` inside the panels (the last-sibling test at ~1402 and
the mini-table needle loop at ~1462) to assert the opposite —
`assert!(!html.contains("lc-flights"))` on both builders' output, with the
comment naming this task. In `http.rs`:

```rust
#[tokio::test]
async fn test_the_flight_layer_lives_in_the_shells_not_the_fragments() {
    // LC rig. Count occurrences of r#"id="lc-flights""# via .matches().count():
    // GET /room/{code}/lastcall (the phone shell)      -> exactly 1
    // GET /room/{code}/screen   (the LC big screen)    -> exactly 1
    // GET /room/{code}/lastcall/hand                   -> 0
    // GET /room/{code}/lastcall/table                  -> 0
    // The last two are the debt's closure: a fetched repaint can no longer
    // contain — and therefore can no longer destroy — the layer.
}

#[tokio::test]
async fn test_a_lagged_subscriber_is_told_to_refetch() {
    // LC rig. Open SSE, drain the snapshot. WITHOUT polling the body further,
    // POST /lastcall/handicap 40 times (each publishes 4 broadcast frames;
    // 160 > the channel's 128 capacity) so the receiver lags. Then:
    // let seen = read_sse_until(&mut body, "data: 0").await;
    // assert!(seen.contains("event: lctick"), "{seen}");
    // Content-filtered: the marker is the synthetic payload itself.
}
```

- [ ] **Step 4: Commit**

```bash
git add drinkinggame/src/routes.rs drinkinggame/src/lc_render.rs drinkinggame/templates/lc_room.html drinkinggame/templates/lc_screen.html drinkinggame/tests/http.rs
git commit -m "fix(lastcall): a lagged SSE receiver re-fetches; the flight layer moves to the static shells"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

### Task 4: The F.1 action bar, the target picker, and `lc_loop.js`

**Class:** B (logic, tests specified below — the bar builder and pane
additions are string builders with exact expected copy; the JS is static and
`node --check`-gated, behaviour eyeballed at checkpoint 1, the Plan C Task 4
precedent)

**Files:**
- Modify: `drinkinggame/src/lc_render.rs` (`ActionBarView`, `lc_action_bar`;
  tests)
- Modify: `drinkinggame/src/lc_routes.rs` (`action_bar_view`,
  `targets_section_html`; `hand_pane_html` appends both; `LcRoomTemplate`
  gains `actions: String`; `lc_page` fills it)
- Modify: `drinkinggame/templates/lc_room.html` (action bar renders the
  field; note div; script tag; two guarded hook lines)
- Modify: `drinkinggame/templates/lc_screen.html` (script tag; one hook line)
- Create: `drinkinggame/assets/lc_loop.js`
- Modify: `drinkinggame/src/routes.rs` (asset handler + route beside
  `lc_motion.js`'s)
- Modify: `drinkinggame/assets/lastcall.css` (hint/note/targets/chip rules)
- Test: `drinkinggame/tests/http.rs`

**Interfaces:**
- Consumes: Task 1's routes and `map_lc`'s 422 message bodies; Task 2's
  `begin` + timer markup; Plan C's `lc:arm`/`lc:disarm` events and
  `hand_pane_html`; `lcFlight`/`lcAnchor` and the `armed` anchor;
  `pull_cost`; `LcOutcome`.
- Produces (Task 5 extends `lc_loop.js`; exact):

```rust
// lc_render.rs — per-viewer, never broadcast (the private-side twin of
// PublicView; rendered only into the shell page and the private hand fetch).
#[derive(Clone, Debug)]
pub struct ActionBarView {
    pub beat: Beat,
    pub round: u32,
    pub seated: bool,
    pub alive: bool,
    pub locked: bool,
    pub drawing: bool,
    pub vessels: Vec<(usize, Deck)>, // (vessel index, deck)
    pub charged: u8,                 // viewer's pulls at the reveal (E9)
    pub vessels_registered: usize,   // players with >= 1 vessel (E1 gate)
    pub outcome: Option<LcOutcome>,
}
pub fn lc_action_bar(ab: &ActionBarView) -> String;

// lc_routes.rs
fn action_bar_view(st: &LastCallState, player_id: i64) -> ActionBarView;
fn targets_section_html(st: &LastCallState, seat: usize) -> String;
// hand_pane_html now returns:
//   {lc_hand_pane(...)}{targets_section}{<template data-lc-actions>bar</template>}
```

```js
// lc_loop.js globals (window.*) — the inline hooks and Task 5 call these:
window.lcLoopApply(pane) // move template[data-lc-actions] into .lc-actions
window.lcLoopPublic()    // arm the beat timer (Task 5 adds flights/hits)
```

- [ ] **Step 1: `lc_action_bar`.** Precedence: `outcome` → `!seated` →
`!alive` → beat. Exact markup per state (whitespace-free, like every builder;
`{DECK}` is `Deck::label()`):

| State | Thumb zone |
| --- | --- |
| `outcome.is_some()` | `<button class="lc-btn lc-btn-drink" data-lc-post="end">END GAME</button>` |
| `!seated` | `<p class="lc-actions-hint">SPECTATING</p>` |
| `!alive` | `<p class="lc-actions-hint">YOU'RE OUT — HAUNT THE TABLE</p>` |
| Draw, round 1, `vessels_registered >= 2` | `<button class="lc-btn lc-btn-drink" data-lc-post="begin">START ROUND 1</button>` |
| Draw, round 1, fewer | `<button class="lc-btn lc-btn-drink" data-lc-post="begin" disabled>START ROUND 1</button><p class="lc-actions-hint">NEEDS 2 DRINKS REGISTERED</p>` |
| Draw, round ≥ 2, `!drawing` | per vessel: `<button class="lc-btn lc-btn-drink" data-lc-post="draw" data-vessel="{idx}">FINISH {DECK} · DRAW</button>` — then `<p class="lc-actions-hint">OR SIT TIGHT</p>` |
| Draw, round ≥ 2, `drawing` | `<p class="lc-actions-hint">FRESH VESSEL — DEALT</p>` |
| Deal | `<p class="lc-actions-hint">DEALING…</p>` |
| Diplomacy | `<p class="lc-actions-hint">TALK IT OUT — DEALS AREN'T BINDING</p>` |
| Lock, `!locked` | `<button class="lc-btn lc-btn-drink" data-lc-post="lock">LOCK IN</button>` |
| Lock, `locked` | `<p class="lc-actions-hint">LOCKED — WAITING FOR THE TABLE</p>` |
| Reveal/Resolve, `charged > 0` | `<div class="lc-btn lc-btn-drink lc-drink-now">DRINK {charged}</div>` |
| Reveal/Resolve, `charged == 0` | `<p class="lc-actions-hint">NOTHING TO PAY</p>` |

F.1 holds by construction: the drinking-adjacent primary is always
`lc-btn-drink` (amber, `lastcall.css:384`), and the beat's decision is the
only thing in the thumb zone. `data-lc-post` is a data-contract attribute
(the same argument as Plan C's CustomEvents) — extend
`test_no_builder_emits_behaviour`'s output list with a populated
`lc_action_bar` call and note there that `data-lc-post` is deliberately not
in the forbidden set (`hx-`, `onclick`, `action=`, hex remain forbidden).

- [ ] **Step 2: route-side assembly.** `action_bar_view`: seat lookup;
`charged` = `st.plays.iter().filter(|p| p.source_seat == seat).map(|p| pull_cost(p.card.cost, handicap_pct)).sum()`
(0 when unseated); `vessels` from the player's vessels; `vessels_registered`
counted over all players. `targets_section_html(st, seat)` — empty string
unless `beat == Lock && !locked` and the player has armed `targets == "one"`
cards; otherwise:

```html
<section class="lc-targets"><h2>Targets</h2>
  <label class="lc-target-row"><span>{title}</span><select data-lc-target data-card-id="{id}">
    <option value="">PICK A TARGET</option>
    <option value="{seat}"[ selected]>{NAME}</option><!-- every Alive seat, selected = the ArmedCard's current target -->
  </select></label><!-- one per armed "one" card -->
</section>
```

(titles/names/ids through `html_escape`, as `lc_hand_pane` does). Append both
to `hand_pane_html`'s return — the fragment's root `#lc-hand` still leads, so
`lcApply`'s `querySelector("#lc-hand")` seq gate is untouched and the extras
ride the same stale-drop. `LcRoomTemplate` gains `actions: String`
(`lc_render::lc_action_bar(&action_bar_view(&ctx.st, player.id))`), and
`lc_room.html`'s bar becomes:

```html
<div id="lc-actions-note" hidden></div>
<div class="lc-actions">{{ actions|safe }}</div>
```

(replacing the two disabled placeholder buttons).

- [ ] **Step 3: `lc_loop.js`.** IIFE + `"use strict"`, the `lc_motion.js`
shape; served by a `routes.rs` handler + route line mirroring `lc_motion_js`;
`<script src="{{ base_path }}/assets/lc_loop.js" defer></script>` in both
shells under the existing script tags. Reads the inline `BP`/`CODE` globals
both templates already define. Contents (constants and contracts verbatim,
wiring follows `lc_motion.js`/`lc_wheel.js` patterns):

```js
var NOTE_MS = 2600, URGENT_MS = 5000;

function post(action, body) {
  return fetch(BP + "/room/" + CODE + "/lastcall/" + action, {
    method: "POST", credentials: "same-origin",
    headers: { "Content-Type": "application/x-www-form-urlencoded" },
    body: body || ""
  }).then(function (res) {
    if (!res.ok) res.text().then(function (t) { note(t || "Not now."); });
    return res.ok;
  });
}
```

- `note(text)`: fill `#lc-actions-note`, unhide, re-hide after `NOTE_MS` —
  this is where `map_lc`'s "Can't afford …" / "… needs a target." bodies
  surface.
- One delegated `click` listener on `document.body` for `[data-lc-post]`
  (skip `disabled`): action = the attribute value; body = `data-vessel`
  present ? `"vessel=" + …` : `""`.
- One delegated `change` listener for `select[data-lc-target]`:
  `post("target", "card_id=" + encodeURIComponent(sel.dataset.cardId) + "&target=" + sel.value)`.
- One `lc:arm` listener on `document.body` (Plan C's contract — delegated
  once, never rebound): `post("arm", "card_id=" + encodeURIComponent(e.detail.cardId))`;
  on success fire the arm flight —
  `window.lcFlight(e.target, window.lcAnchor("armed"), { direction: "play", scale: "dot", deck: e.target.querySelector(".lc-cardface") && e.target.querySelector(".lc-cardface").dataset.deck })`.
  One `lc:disarm` listener posting `disarm`, no flight.
- `window.lcLoopApply(pane)`: find `template[data-lc-actions]` in `pane`,
  replace `.lc-actions`' innerHTML with its content, remove the template.
- `window.lcLoopPublic()`: find `#lc-beat-timer[data-deadline-ms]`; set
  `--lc-beat-ms` to `Math.max(0, deadline - Date.now()) + "ms"`; clear any
  pending urgency timer and schedule `classList.add("is-urgent")` at
  `remaining - URGENT_MS` (immediately if already inside it).
- Bind on `DOMContentLoaded` (run `lcLoopPublic()` once for the
  server-rendered banner) with a `window.__lcLoopBound` double-injection
  guard. No live resources are held — nothing to release.

Inline hooks (decision E14 — the logic above lives in the checked asset; the
inline scripts get only these guarded lines, and each carries a one-line
comment noting the STATUS-recorded silent-failure property of inline
`<script>`):
- `lc_room.html` `lcApply`, after the Plan C `lcWheelInit` line:
  `if (window.lcLoopApply) window.lcLoopApply(pane);`
- `lc_room.html` `lcpublic` listener, after the banner swap:
  `if (window.lcLoopPublic) window.lcLoopPublic();`
- `lc_screen.html` `lcpublic` listener, after the panel swap:
  `if (window.lcLoopPublic) window.lcLoopPublic();`

- [ ] **Step 4: CSS**, in the shell section of `lastcall.css` (tokens exist;
no new colours):

```css
/* Plan E — the action bar's states (decision E7). */
.lc-actions-hint { flex: 1; align-self: center; text-align: center;
                   font-family: var(--font-ui); font-weight: 700; font-size: 11px;
                   letter-spacing: .12em; text-transform: uppercase;
                   color: var(--lc-faint); }
#lc-actions-note { padding: 4px 14px; text-align: center;
                   font-family: var(--font-mono); font-size: 11px;
                   color: var(--lc-rose); }
.lc-drink-now { display: flex; align-items: center; justify-content: center;
                cursor: default; }
.lc-targets { padding: 0 14px 10px; }
.lc-target-row { display: flex; align-items: center; justify-content: space-between;
                 gap: 10px; padding: 6px 0; font-size: 13px; }
```

- [ ] **Step 5: tests.** In `lc_render.rs` (`ActionBarView` fixtures built
inline):

```rust
#[test]
fn test_action_bar_states() {
    // One assertion pair per row of the Step 1 table — the exact copy string
    // present, and the states it must NOT show absent, e.g.:
    //   lobby(1 vessel):  contains "START ROUND 1" AND " disabled"
    //                     AND "NEEDS 2 DRINKS REGISTERED"
    //   draw r2, vessels [(0, Beer), (1, Soft)]: contains
    //     r#"data-vessel="0">FINISH BEER · DRAW"# and
    //     r#"data-vessel="1">FINISH SOFT · DRAW"#
    //   lock unlocked:    contains "LOCK IN", not "LOCKED — WAITING"
    //   reveal charged 3: contains "DRINK 3", class lc-btn-drink (amber, F.1)
    //   outcome Winner:   contains "END GAME", and outcome wins over beat
    //   eliminated:       "YOU'RE OUT — HAUNT THE TABLE" even at Lock
    // no_hex on every variant.
}
```

In `http.rs`:

```rust
#[tokio::test]
async fn test_hand_fetch_carries_the_action_template_per_viewer() {
    // Task 1 rig at Lock; alice locks (via routes), bob does not.
    // GET hand as alice: contains "<template data-lc-actions>" and
    //   "LOCKED — WAITING FOR THE TABLE".
    // GET hand as bob: contains "LOCK IN" and NOT "LOCKED — WAITING".
}

#[tokio::test]
async fn test_target_picker_lists_only_alive_seats_and_posts_back() {
    // alice arms beer-01 ("one"); rig cara eliminated. GET hand as alice:
    // .lc-targets present, contains bob's name, NOT cara's; POST target
    // card_id=beer-01&target=1 -> 204; re-GET: that option now " selected".
}

#[tokio::test]
async fn test_reveal_charge_is_priced_per_viewer() {
    // alice (handicap 100) and bob both play the same cost-2 card; advance
    // to Reveal via both locking. GET hand: alice's bar "DRINK 2"; rig bob's
    // handicap 150 before locking -> bob's bar "DRINK 3". Same card, two
    // prices — mirrors Plan C's per-viewer rail test at the loop level.
}
```

- [ ] **Step 6: Commit**

```bash
git add drinkinggame/src/lc_render.rs drinkinggame/src/lc_routes.rs drinkinggame/src/routes.rs drinkinggame/templates/lc_room.html drinkinggame/templates/lc_screen.html drinkinggame/assets/lc_loop.js drinkinggame/assets/lastcall.css drinkinggame/tests/http.rs
git commit -m "feat(lastcall): the F.1 action bar — beat decisions in the thumb zone, target picker, lc_loop.js"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

## Browser checkpoint 1 — after Task 4

A human, a real focused browser (automation tabs freeze animation),
`cargo run -p drinkinggame`, two profiles logged in, a room, START on the Last
Call card:

1. Register drinks in both sessions; the bar shows START ROUND 1 disabled
   until the second vessel, then enabled; press it — banner flips to
   DIPLOMACY, the timer rail starts draining, and after 60s the beat advances
   to LOCK on its own.
2. At LOCK: tap the focused wheel card — it flies to the armed column and
   ARMED ticks; the target picker lists the other player; pick, LOCK IN. The
   other phone still shows LOCK IN; lock it too — the beat advances to REVEAL
   immediately (all locked), before its timer.
3. Arm more than a vessel can afford — the 422 note appears with the card's
   name and nothing changes.
4. At REVEAL both phones show their DRINK n; after 20s the round rolls over
   to DRAW round 2 with FINISH … · DRAW in the thumb zone.
5. The timer turns rose inside the last 5 seconds; under devtools' "Emulate
   prefers-reduced-motion: reduce" the rail holds at rest and the game still
   advances.
6. Kill a player over a few rounds: their phone shows YOU'RE OUT; at game
   over both phones show END GAME, pressing it lands everyone back on the
   idle room panel (the `.game-idle` handoff — watch it actually navigate:
   this path is inline-script, no harness reaches it).

---

### Task 5: Reveal, damage and game-over presentation

**Class:** B (string builders with exact expected values plus static JS;
nothing here locks, gates or publishes — the broadcast paths are Tasks 1–3's)

**Files:**
- Modify: `drinkinggame/src/lc_render.rs` (`lc_screen_panel` centre;
  `lc_banner` game-over branch; tests — including replacing
  `test_the_felt_centre_holds_no_plays`)
- Modify: `drinkinggame/assets/lc_loop.js` (extend `lcLoopPublic`)
- Modify: `drinkinggame/assets/lastcall.css` (centre-plays + victory rules;
  reduced-motion additions inside the existing block)
- Test: `drinkinggame/tests/http.rs` (one end-to-end reveal frame test)

**Interfaces:**
- Consumes: `PublicView.revealed` (order_key-sorted by Plan D's reveal),
  `PublicView.outcome`, `card_mini`, `html_escape`, `lcFlight`/`lcAnchor`,
  the `seat-{n}`/`felt`/`deck-{deck}` anchors (all shipped since Plan B),
  `.lc-plaque.is-hit` (`lastcall.css:459`).
- Produces: the felt-centre reveal stack, the game-over banner/centre, and
  the client-side motion pass.

- [ ] **Step 1: the felt centre (decision E15).** In `lc_screen_panel`'s
stage, between `#lc-felt` and the ring: when `view.revealed` is non-empty,

```html
<div class="lc-centre-plays">
  <div class="lc-centre-play" data-seat="{source_seat}">
    <span class="lc-centre-cap">{SRC} → {TGT}</span>{card_mini(&play.card)}
  </div><!-- one per play, in the vec's (order_key) order -->
</div>
```

where `SRC` is the source seat's name uppercased and `TGT` is the target
seat's name uppercased, or the card's `targets` value uppercased (`ALL` /
`TABLE`) when `play.target` is `None` — names via `html_escape`, resolved
from `view.seats`. When `view.outcome` is `Some`, render instead:

```html
<div class="lc-centre-victory">{NAME} OUTLASTS THE TABLE</div>
<!-- or, for LcOutcome::Draw: -->
<div class="lc-centre-victory">EVERYBODY'S OUT</div>
```

`lc_banner`: when `view.outcome` is `Some`, the label is `GAME OVER`, the hue
class is `lc-beat-rose`, the meta is `ROUND {round} · LAST CALL`, and no
timer is emitted (the deadline is `None` anyway). **The mini table's centre
stays the decorative pile** — the phone's reveal is the banner plus the DRINK
chip (E15); note this in the builder comment.

- [ ] **Step 2: the motion pass.** Extend `lcLoopPublic()` in `lc_loop.js`
with a module-level `prev = { beat: null, hp: {}, draws: {} }` snapshot,
re-read from the DOM after every call (plaques' `data-seat`/`data-hp`,
mini-table chips likewise; the banner's `data-beat`):

- **Reveal flights:** when the beat flips to `"reveal"`, for each
  `.lc-centre-play` (big screen), fire
  `lcFlight(lcAnchor("seat-" + seat), lcAnchor("felt"), { direction: "play", deck: mini.dataset.deck, delay: i * 220 })`
  — E.1's 0.2–0.3s stagger. Guard every flight with
  `el.offsetParent !== null` on both anchors: the phone's TABLE pane is
  usually `hidden`, and a flight between zero-rect anchors is garbage — fire
  only on visible surfaces (decision E17; the big screen always qualifies).
- **Draw flights:** for each seat whose `data-draws` (already on the plaque
  via `PublicSeat.draws`) increased, fire deck→seat
  (`lcAnchor("deck-" + firstDeckSlug)` → seat anchor), same visibility guard.
- **Hits:** for each seat whose `data-hp` decreased, add `is-hit` to its
  `.lc-plaque` and remove it on `animationend` — `lc-shake` + `lc-hp-flash`
  run (`lastcall.css:459-460`). Mini-table chips have no `is-hit` rule; skip
  them (the HP number itself repaints).
- Reduced motion needs no JS branch: `lcFlight` already collapses to
  `onArrive` under `reduced()`, and the `is-hit` animations are inside the
  existing reduced-motion block.

The plaques carry `data-hp`/`data-draws` from Plan A; verify and, if either
attribute is missing from `player_plaque`'s output, add it there (a
contract-completing attribute, not a new component) with the existing plaque
tests extended to pin it.

- [ ] **Step 3: CSS**, in the screen section (no new colours; reduced-motion
addition inside the existing block):

```css
/* Plan E — the reveal on the felt (decision E15). */
.lc-centre-plays { position: absolute; inset: 0; display: flex; gap: 14px;
                   align-items: center; justify-content: center;
                   pointer-events: none; }
.lc-centre-play { display: flex; flex-direction: column; align-items: center;
                  gap: 6px; }
.lc-centre-cap { font-family: var(--font-ui); font-weight: 700; font-size: 11px;
                 letter-spacing: .14em; color: var(--lc-faint); }
.lc-centre-victory { position: absolute; inset: 0; display: flex;
                     align-items: center; justify-content: center;
                     font-family: var(--font-display); font-weight: 900;
                     font-size: 40px; letter-spacing: -.02em;
                     color: var(--lc-text); }
```

(`.lc-stage` is already `position: relative`, `lastcall.css:736` — the two
absolute layers sit on the felt.)

- [ ] **Step 4: tests.** In `lc_render.rs`:

```rust
#[test]
fn test_the_felt_centre_shows_revealed_plays_in_order() {
    // REPLACES test_the_felt_centre_holds_no_plays — that test pinned the
    // slice-1 boundary ("someone has begun slice 3 inside Plan B"); slice 3
    // is now deliberately here. Secrecy no longer rests on the renderer:
    // public_view() only populates `revealed` at Reveal/Resolve, pinned by
    // last_call.rs's own projection tests (Plan D's mandatory §3.4.1 test).
    // ring_fixture(4) + two revealed plays, order_key 1 and 2, one targeted
    // (Some(2)) and one "all" (None):
    //   - both card titles present; find() position of play 1's title <
    //     play 2's (vec order is render order);
    //   - the captions: "SEAT-1-NAME → SEAT-2-NAME" style pair present, and
    //     the None-target play's caption ends "→ ALL";
    //   - revealed empty -> no "lc-centre-plays" at all;
    //   - no_hex.
}

#[test]
fn test_game_over_takes_over_banner_and_centre() {
    // view.outcome = Some(Winner(1)): banner contains "GAME OVER" and
    // "lc-beat-rose", no data-deadline-ms; screen panel contains
    // "{NAME} OUTLASTS THE TABLE" and no lc-centre-plays.
    // outcome Draw -> "EVERYBODY'S OUT".
}
```

In `http.rs`:

```rust
#[tokio::test]
async fn test_the_reveal_frame_carries_the_plays_and_the_end_still_hands_off() {
    // Task 1 rig; alice targets + locks a play, bob locks nothing; the
    // all-locked advance flips to Reveal. read_sse_until "event: lcpublic":
    // the frame contains lc-centre-plays and the played card's title —
    // identity is public exactly now. Then POST /lastcall/end and
    // read_sse_until "event: game": matches_the_game_idle_selector(frame) —
    // the existing handoff (test_ending_last_call_keeps_the_room_open's
    // property) still holds with the loop live.
}
```

- [ ] **Step 5: Commit**

```bash
git add drinkinggame/src/lc_render.rs drinkinggame/assets/lc_loop.js drinkinggame/assets/lastcall.css drinkinggame/tests/http.rs
git commit -m "feat(lastcall): the reveal on the felt — ordered plays, flights, hits, and the game-over tableau"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

## Browser checkpoint 2 — after Task 5, before the final review

Same two-profile setup, plus the big screen (`/room/{CODE}/screen`) on a third
window, and a phone with the TABLE tab open:

1. Play a round to the reveal watching the big screen: flights travel from
   the locking seats to the felt centre with the ~220ms stagger, the plays
   land as minis with SRC → TGT captions in spend order, and the hit plaques
   shake with the HP flash.
2. The phone with TABLE open shows dot-scale flights; the phone on HAND shows
   none and nothing errors (the visibility guard).
3. A mid-flight repaint (have the other player act during the stagger) does
   not vanish the flying cards — the Task 3 layer move, eyeballed.
4. Finish a game: GAME OVER banner + victory line on the screen, END GAME on
   the phones, and the handoff back to the idle room panel works from both a
   phone and the spectator screen.
5. Repeat step 1 under "Emulate prefers-reduced-motion: reduce": no flights,
   but every number still moves (arrivals fire without animation).

## Before the plan is done

- Every task's `./scripts/verify.sh` output quoted; tests grow from the
  post-C/D baseline; clippy still **17 distinct** warnings, all
  `drawingportfolio`; `drinkinggame` stays clean.
- Both browser checkpoints run by a human.
- Class C tasks (1, 2, 3) each had their per-task reviewer on a capable model
  (plan-economics §3/§4); Tasks 4–5 are covered by **one whole-plan review**
  of the branch diff at the end, on the most capable model. The reviewer
  brief must name: the tick-only vs full-publish split (E5), the
  ticker/route double-advance race and its relock-recheck answer (Task 2),
  the `hand_len` secrecy projection (E6), `broadcast_lc`/`persist_and_tick_lc`
  staying await-free under the guard, the flight-layer move's containing
  blocks, and the inline-hook lines (the same silent-failure territory STATUS
  records for `.game-idle` — neither `node --check` nor any harness reaches
  them).
- No `cargo sqlx prepare` (no migration).
- Interfaces line up: Task 1's routes are what Task 4's JS posts; Task 2's
  `lc_advance_chain` is what Task 1's lock handler and the ticker both call;
  Task 3's static layer is what Task 5's flights render into; the E7 copy
  strings in Task 4's builder are the ones its tests and Task 4's http tests
  assert.
- STATUS updated: slice 3b shipped and playable; debts 1 (flight layer) and
  2 (lag arm) closed with their tests named; decision E3's deferred ready
  affordances, E15's deferred discard flights and the Plan J end-screen
  handoff recorded as open; the `from_json` seat cap noted as closed by Plan
  D Task 1 (pre-deploy item retired) — verify before writing, don't assume.

## Self-review (performed while writing)

- **Scope coverage vs the brief:** action routes under the room guard ✔
  (Task 1, tm_routes shape, RNG in the route per D6); action bar +
  `lc:arm`/`lc:disarm` consumption ✔ (Task 4, delegated once per Plan C's
  contract, amber rule by construction); beat clock on the existing ticker ✔
  (Task 2, storage decision E2 documented, engine clock-free); reveal/resolve
  presentation + E.1 flights + `armed` anchor ✔ (Tasks 4–5); game-over via
  the existing `lc_end_handler` with the minimal summary here and the full
  screen named as Plan J ✔; SSE order/await-free/no-new-event preserved ✔
  (E5 justified against "who is subscribed"); flight-layer debt closed
  structurally with owner named ✔ (Task 3); lag arm decided (re-fetch) and
  tested ✔ (Task 3); `.game-idle` inline pattern not extended — new logic in
  `lc_loop.js`, hooks are guarded one-liners with the property noted ✔;
  RoF/3 Man regression pins named per route/ticker change ✔.
- **Placeholder scan:** no TBD/TODO/"handle errors appropriately"; every
  route path, form field, copy string, status code, constant and test
  expectation is spelled out; prose only where a named pattern exists
  (`tm_roll_handler`, `lc_motion.js`, the Task 1 handler skeleton).
- **Type consistency:** Plan D signatures consumed verbatim (checked against
  Plan D's Produces blocks — `finish_and_draw(player_id, vessel_idx, drawn)`,
  `LcError`'s twelve live variants post-`NotImplemented`, `LcOutcome`,
  `duration_secs -> Option<u16>`); Plan C's `hand_pane_html` extended, not
  forked, with the C/D `ArmedCard` seam handled in Global Constraints;
  `beat_deadline_ms` flows engine-field → projection → `beat_timer_live` →
  `lc_loop.js`; `ActionBarView` is built in `lc_routes` and rendered in
  `lc_render`, never broadcast.
- **Known cross-plan risk, flagged upward:** Plan C's CostRail root class
  `.lc-rail` collides with the big screen's existing `.lc-rail` rails
  (`lastcall.css:700`, `lc_render.rs:510`). Not this plan's file to fix — if
  Plan C's review has not renamed one side by execution time, checkpoint 1
  will show it on the phone and the fix belongs to Plan C's fix wave.
