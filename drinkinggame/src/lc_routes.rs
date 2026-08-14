//! Last Call route handlers. `/lastcall/start`, `/lastcall/vessel` and
//! `/lastcall/handicap` land here (Task 1); the shell page, hand fragment
//! and the beat-loop action routes are later tasks. SQL stays in db.rs; HTML
//! fragments stay in lc_render.rs.

use askama::Template;
use axum::extract::{Form, Path, State};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Redirect};
use rand::Rng;
use serde::Deserialize;

use crate::auth::PlayerSession;
use crate::db;
use crate::error::GameError;
use crate::last_call::{
    Beat, Card, CardKind, Deck, EffectOp, LastCallState, LcError, Play, PublicView, Status,
    DRAW_PER_VESSEL,
};
use crate::lc_render::{self, ActionBarView, HandGroupView};
use crate::models::{Game, Player, Room};
use crate::GameState;

/// Everything an action handler needs to read and mutate a running Last Call
/// game: the room (for id/code), the raw `games` row (for its id), and the
/// parsed state (mutated in place by the handler, then persisted). Mirrors
/// `tm_routes::TmCtx`.
pub(crate) struct LcCtx {
    pub room: Room,
    pub game: Game,
    pub st: LastCallState,
}

/// member_room -> active game -> kind == "last_call" else WrongGameKind ->
/// parse state. Shared entry point for every `/lastcall/*` handler, in the
/// exact shape of `tm_routes::load_tm`.
pub(crate) async fn load_lc(
    state: &GameState,
    code: &str,
    player: &Player,
) -> Result<LcCtx, axum::response::Response> {
    let room = crate::game::member_room(state, code, player).await?;
    let Some(game) = db::get_active_game(&state.pool, room.id).await else {
        return Err(GameError::NoActiveGame.into_response());
    };
    if game.kind != "last_call" {
        return Err(GameError::WrongGameKind.into_response());
    }
    let st = LastCallState::from_json(game.state_json.as_deref().unwrap_or_default());
    Ok(LcCtx { room, game, st })
}

/// Persists the mutated state back to the DB, then re-renders and publishes
/// every surface that reflects it: the phone GAME tab (`broadcast_game`),
/// the ROOM/TABLE panel (`broadcast_room` — mode = "last_call" for as long
/// as this game's phone panel is still the Task 1 placeholder), and finally
/// the LcPublic / LcTick publishes. `broadcast_game` must run: without it,
/// pressing START is a complete visual no-op on every phone (plan-end
/// review finding I1) — `LcPublic`/`LcTick` only reach clients already on
/// the Last Call shell, and nobody is there yet at the instant a game
/// starts. Order mirrors `tm_routes::persist_and_broadcast` (game, then
/// room, then the game-specific publish). `broadcast_lc` runs after
/// `set_game_state` (so a phone that fetches on the tick reads the
/// persisted state) and, like the other two, while the caller's room lock
/// is still held — every caller takes the guard around this whole call, and
/// releasing it first would let a concurrent handler's broadcast land after
/// this one and leave this request's stale render as the last word
/// (`1e742d4`).
pub(crate) async fn persist_and_broadcast_lc(state: &GameState, ctx: &LcCtx) {
    db::set_game_state(&state.pool, ctx.game.id, &ctx.st.to_json()).await;
    db::touch_room(&state.pool, ctx.room.id).await;
    crate::game::broadcast_game(state, ctx.room.id, &ctx.room.code, None).await;
    crate::game::broadcast_room(state, ctx.room.id, &ctx.room.code).await;
    broadcast_lc(state, ctx.room.id, &ctx.st).await;
}

/// Publishes the public fragment and then the tick. Both make every phone
/// re-fetch its own hand; the client coalesces the pair into one fetch, and
/// the stale-drop rule makes a duplicate harmless. Two messages rather than
/// one because the spectator screen consumes only `LcPublic` and later
/// private-only transitions (arming a card) will publish only `LcTick`.
pub(crate) async fn broadcast_lc(state: &GameState, room_id: i64, st: &LastCallState) {
    let view = st.public_view();
    state.hub.publish(
        room_id,
        crate::hub::RoomMessage::LcPublic(lc_render::lc_public_panel(&view)),
    );
    state
        .hub
        .publish(room_id, crate::hub::RoomMessage::LcTick(view.seq));
}

/// Resolves the room and hands back its per-room lock — acquired by the
/// caller so everything from re-load through persist runs under one guard.
/// Mirrors `tm_routes::tm_lock`.
async fn lc_lock(
    state: &GameState,
    code: &str,
) -> Result<std::sync::Arc<tokio::sync::Mutex<()>>, axum::response::Response> {
    let Some(room) = db::get_open_room(&state.pool, &code.to_uppercase()).await else {
        return Err(GameError::RoomNotFound.into_response());
    };
    Ok(state.locks.for_room(room.id))
}

/// member_room -> room_members (>= 2 else TooFewPlayers) ->
/// LastCallState::new(members, rng_seed) -> start_game -> touch, broadcast
/// room. Locked across the whole body: a concurrent join mutating the room's
/// member list between the count check and the seed must not race a start.
/// Mirrors `tm_routes::tm_start_handler`.
pub async fn lc_start_handler(
    State(state): State<GameState>,
    PlayerSession(player): PlayerSession,
    Path(code): Path<String>,
) -> axum::response::Response {
    let room = match crate::game::member_room(&state, &code, &player).await {
        Ok(r) => r,
        Err(resp) => return resp,
    };
    let lock = state.locks.for_room(room.id);
    let _guard = lock.lock().await;

    let members = db::room_members(&state.pool, room.id).await;
    if members.len() < 2 {
        return GameError::TooFewPlayers.into_response();
    }
    // last_call.rs never generates its own randomness; the seed is taken
    // once, here, and stored in the state blob for the shuffle/deal math it
    // does with it. `lc_draw_handler` (below) is a second, independent
    // rand::thread_rng() site — it samples drawn cards per request rather
    // than seeding persisted state, so it doesn't go through this seed.
    let rng_seed = rand::thread_rng().gen::<u64>();
    let st = LastCallState::new(
        members.iter().map(|m| (m.id, m.name.clone())).collect(),
        rng_seed,
    );
    // deck_order/rules_json are Ring of Fire concepts — Last Call leaves
    // both empty, same as 3 Man. state_json must be Some(...): from_json
    // expects valid JSON and "" is not valid JSON (Task 1, Step 1).
    // GameAlreadyActive races are handled by the games table's partial
    // unique index (one active game per room).
    if let Err(e) = db::start_game(
        &state.pool,
        room.id,
        "last_call",
        "",
        "",
        Some(&st.to_json()),
    )
    .await
    {
        return e.into_response();
    }

    // Re-load under the lock rather than hand-assembling a Game/LcCtx: this
    // doubles as proof persist_and_broadcast_lc works for a freshly-started
    // game, the same helper every future action handler reuses.
    let ctx = match load_lc(&state, &code, &player).await {
        Ok(c) => c,
        Err(resp) => return resp,
    };
    persist_and_broadcast_lc(&state, &ctx).await;
    StatusCode::NO_CONTENT.into_response()
}

/// `POST /room/{code}/lastcall/end`. Ends the game, not the room — the room
/// stays open for another game to start on it. Modelled on
/// `tm_routes::tm_end_handler` line for line: member_room -> lock -> load_lc
/// -> `db::end_game` -> `db::touch_room` -> publish `Game`/`Screen` ->
/// `broadcast_room`.
///
/// The `Screen` frame is built via `game::current_screen_panel`, not a
/// direct `render::` call: `db::end_game` has already run, so
/// `db::get_active_game` now returns `None` for this room, and
/// `current_screen_panel`'s own kind-branch falls through to
/// `render::screen_panel_idle` on its own — the same "kind-aware for free"
/// property `tm_end_handler`'s comment documents for its closing
/// `broadcast_leaderboard` call. That idle panel carries no `data-lc-live`
/// marker, which is what sends every spectator already on `lc_screen.html`
/// back to the generic `screen.html` (Task 4's handoff, run in reverse).
pub async fn lc_end_handler(
    State(state): State<GameState>,
    PlayerSession(player): PlayerSession,
    Path(code): Path<String>,
) -> axum::response::Response {
    let room = match crate::game::member_room(&state, &code, &player).await {
        Ok(r) => r,
        Err(resp) => return resp,
    };
    let lock = state.locks.for_room(room.id);
    let _guard = lock.lock().await;

    let ctx = match load_lc(&state, &code, &player).await {
        Ok(c) => c,
        Err(resp) => return resp,
    };
    db::end_game(&state.pool, ctx.game.id).await;
    db::touch_room(&state.pool, room.id).await;

    let phone_html = crate::game::idle_panel(&state, &room.code).await;
    let screen_html = crate::game::current_screen_panel(&state, room.id, &room.code).await;
    state
        .hub
        .publish(room.id, crate::hub::RoomMessage::Game(phone_html));
    state
        .hub
        .publish(room.id, crate::hub::RoomMessage::Screen(screen_html));
    crate::game::broadcast_room(&state, room.id, &room.code).await;
    StatusCode::NO_CONTENT.into_response()
}

/// `POST /room/{code}/lastcall/rematch`. J8: any member may call it, gated
/// on the finished game actually being over — `ctx.st.outcome().is_none()`
/// maps to the same 409 `OutOfTurn` every other "not now" case in this file
/// uses. From `db::end_game` down this is `lc_start_handler`'s body (member-
/// count check, fresh `LastCallState::new`, `db::start_game`, re-`load_lc`,
/// `persist_and_broadcast_lc`, 204) with one deliberate departure — see the
/// `st.seq` line below — under the one room lock acquired below, so no
/// ticker tick and no concurrent action (including a second REMATCH tap)
/// can land between the old game ending and the new one starting. Unlike
/// `lc_end_handler`, this never touches `idle_panel`/`current_screen_panel`
/// or publishes a bare `Game`/`Screen` frame — `persist_and_broadcast_lc`'s
/// `broadcast_game`/`broadcast_room`/`broadcast_lc` trio is the entire
/// publish, exactly as it is for `lc_start_handler`, because nobody needs
/// to leave the Last Call shell: every phone and the big screen are still
/// subscribed to the same room and simply repaint into round 1.
///
/// Review fix round 1 (C1, plan erratum — the brief's "start flow verbatim"
/// prescription had this defect, not the implementation): a bare
/// `LastCallState::new` starts `seq` at 0, which is exactly what
/// `lc_start_handler` wants (nobody is on the shell yet when a game starts
/// fresh — `lc_room.html`'s `lcSeq` and `lc_screen.html`'s twin both seed
/// from the page that redirected them there). REMATCH is different: J8
/// keeps every phone on the shell and the big screen on the felt, and each
/// already holds the FINISHED game's seq as its stale-drop floor
/// (`lcApply`/`lcApplyTable` in `lc_room.html`, the `lcpublic` frame check
/// in `lc_screen.html`). A fresh game restarting at seq 0 would land below
/// that floor and get silently dropped by every one of them, forever (no
/// `.game-idle` fires either, since nobody left the room) — the seq counter
/// is scoped to the ROOM's connected clients, not to any one game, so it
/// has to carry forward across the end/start boundary instead of resetting.
pub async fn lc_rematch_handler(
    State(state): State<GameState>,
    PlayerSession(player): PlayerSession,
    Path(code): Path<String>,
) -> axum::response::Response {
    let room = match crate::game::member_room(&state, &code, &player).await {
        Ok(r) => r,
        Err(resp) => return resp,
    };
    let lock = state.locks.for_room(room.id);
    let _guard = lock.lock().await;

    let ctx = match load_lc(&state, &code, &player).await {
        Ok(c) => c,
        Err(resp) => return resp,
    };
    if ctx.st.outcome().is_none() {
        return GameError::OutOfTurn.into_response();
    }

    // Review fix round 1, minor 1: checked before `db::end_game` (was
    // after) so a failure here never ends the finished game while starting
    // nothing. Currently unreachable either way — no leave/kick route
    // exists, and a running game already implies >= 2 members — but free to
    // get right while this function is already open for C1.
    let members = db::room_members(&state.pool, room.id).await;
    if members.len() < 2 {
        return GameError::TooFewPlayers.into_response();
    }
    db::end_game(&state.pool, ctx.game.id).await;

    let rng_seed = rand::thread_rng().gen::<u64>();
    let mut st = LastCallState::new(
        members.iter().map(|m| (m.id, m.name.clone())).collect(),
        rng_seed,
    );
    // C1: carry the room's stale-drop floor across the game boundary — see
    // the doc comment above.
    st.seq = ctx.st.seq + 1;
    if let Err(e) = db::start_game(
        &state.pool,
        room.id,
        "last_call",
        "",
        "",
        Some(&st.to_json()),
    )
    .await
    {
        return e.into_response();
    }

    let ctx = match load_lc(&state, &code, &player).await {
        Ok(c) => c,
        Err(resp) => return resp,
    };
    persist_and_broadcast_lc(&state, &ctx).await;
    StatusCode::NO_CONTENT.into_response()
}

#[derive(Deserialize)]
pub struct VesselForm {
    pub deck: String,
    // Screen-declutter pack (2026-08-13): the container field left the UI
    // (people drink from their own glasses). Defaulted so the route stays
    // wire-compatible with anything still posting one.
    #[serde(default)]
    pub container: String,
}

/// Registers what a seated player is drinking. Follows `tm_mode_handler`'s
/// gate-then-validate shape: `load_lc` (member/active-game/kind gating) runs
/// BEFORE the form's own field validation, so a non-member or a wrong-kind
/// game gets its 403/409 rather than a 422 that would leak "this form field
/// is invalid" to a request with no business hitting this room at all.
///
/// `set_vessel`'s own errors (D15: `NotSeated`/`NotAlive`/`WrongBeat` — the
/// Draw-beat gate) go through `map_lc`, the same mapping every Plan E action
/// route uses (fix round 1, Plan E Task 1 review) — a blanket 422 here
/// previously disagreed with `lc_handicap_handler` a few lines down, which
/// already mapped its own `WrongBeat`/`NotSeated` through `map_lc`'s 409/403.
pub async fn lc_vessel_handler(
    State(state): State<GameState>,
    PlayerSession(player): PlayerSession,
    Path(code): Path<String>,
    Form(form): Form<VesselForm>,
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

    let Some(deck) = Deck::from_slug(&form.deck) else {
        return StatusCode::UNPROCESSABLE_ENTITY.into_response();
    };
    let container = form.container.trim();
    // chars().count(), not bytes: matches `rule_handler`'s convention for
    // user-entered text length limits elsewhere in this crate.
    if container.chars().count() > 24 {
        return StatusCode::UNPROCESSABLE_ENTITY.into_response();
    }

    if let Err(e) = ctx.st.set_vessel(player.id, deck, container) {
        return map_lc(e);
    }
    persist_and_broadcast_lc(&state, &ctx).await;
    // J13: `ctx.room.code` is the canonical (uppercase) code `member_room`
    // resolved, not the raw `code` path segment a lowercase-cased link could
    // carry — a Plan A2 minor closed here.
    Redirect::to(&format!(
        "{}/room/{}/lastcall",
        state.base_path, ctx.room.code
    ))
    .into_response()
}

#[derive(Deserialize)]
pub struct HandicapForm {
    pub target: i64,
    pub handicap_pct: u16,
}

/// Sets a seat's handicap. **Not owner-scoped** — spec §2, item 2: any room
/// member may set any player's handicap, deliberately, because the table
/// sets handicaps rather than each player declaring themselves a
/// lightweight (mirrors `presets.rs`'s "not owner-scoped — it's a friends
/// app" model). `handicap_pct: u16` in the form struct is doing real work:
/// it rejects negatives, decimals, `NaN` and `inf` at Form extraction, before
/// this handler body ever runs — spec §6.1's "remove the input rather than
/// check it" pattern applied to a scalar. Both `LcError::BadHandicap`
/// (out-of-range) and `LcError::NotSeated` (the *target* isn't in this game)
/// map to the same 422: from the caller's point of view both are "that's not
/// a settable handicap for anyone in this room right now."
///
/// `LcError::WrongBeat` is its own branch, added alongside D19 (which
/// Draw-beat-gates `set_handicap`, closing the review's I1 pre-D19-decision
/// finding): a handicap raised after Lock is "not now", the same 409
/// `map_lc` gives every other WrongBeat case — folding it into the 422
/// bucket would tell a caller retrying mid-round that 150 was an invalid
/// percentage rather than that the window had closed.
pub async fn lc_handicap_handler(
    State(state): State<GameState>,
    PlayerSession(player): PlayerSession,
    Path(code): Path<String>,
    Form(form): Form<HandicapForm>,
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

    if let Err(e) = ctx.st.set_handicap(form.target, form.handicap_pct) {
        return match e {
            LcError::WrongBeat => GameError::OutOfTurn.into_response(),
            _ => StatusCode::UNPROCESSABLE_ENTITY.into_response(),
        };
    }
    persist_and_broadcast_lc(&state, &ctx).await;
    // J13: same canonical-code fix as `lc_vessel_handler` above.
    Redirect::to(&format!(
        "{}/room/{}/lastcall",
        state.base_path, ctx.room.code
    ))
    .into_response()
}

/// The single builder of the `#lc-hand` fragment — mirrors
/// `table_pane_html`'s role, so the shell's initial paint (`lc_page`) and the
/// per-viewer refetch (`lc_hand_handler`) can never disagree about the
/// fragment's shape for the same state. Closes the STATUS-carried
/// "rows-and-hand lookup duplicated verbatim" minor from Plan A2.
///
/// Plan E Task 4: the armed column's card source now branches on `locked` —
/// `lock_in` empties `p.armed` into `locked_plays` (§3.4.1), so reading
/// `p.armed` unconditionally left a just-locked viewer's own `LOCKED {n}`
/// header showing zero minis (the seam Plan D's review flagged and left for
/// this task to close). `st.staged_for(seat)` is the wired fix: it reads
/// `locked_plays` filtered to this seat, exactly the cards `lock_in` just
/// moved there. Also appends the F.1 action bar (inside a `<template>`, so
/// it never paints where it sits — `lcLoopApply` relocates it into
/// `.lc-actions`) and, when applicable, the Lock-beat target picker — both
/// OUTSIDE `#lc-hand` itself, so `lcApply`'s `querySelector("#lc-hand")` seq
/// gate is untouched and the extras ride the same stale-drop.
///
/// Plan H Task 5 (H13): also appends the private `.lc-tabcard` panel — the
/// one surface tab identity may render on, gated on the viewer's own seat
/// being `Status::Alive` (an unseated spectator never held a tab; an
/// Eliminated one had theirs voided at elimination, H10). Rides the same
/// private fetch, seq gate and stale-drop as the target picker above it.
///
/// Plan J Task 3: once `st.outcome()` is `Some`, the pane body switches
/// wholesale to `lc_end_card` — the register row, response window, tab
/// drawer, inspect sheet and mulligan overlay all stop applying to a
/// finished game (their gates — `table_pane_html`'s staging window, the
/// overlay's Draw window — are already moot once the beat is frozen at
/// Resolve). The root id and `data-seq` stay put
/// so `lcApply`'s `querySelector("#lc-hand")` stale-drop gate keeps working
/// unchanged, and the `<template data-lc-actions>` sibling is still
/// appended — `lc_action_bar`'s own `outcome.is_some()` branch (REMATCH /
/// END NIGHT) supplies its content.
fn hand_pane_html(base_path: &str, code: &str, st: &LastCallState, player_id: i64) -> String {
    if st.outcome().is_some() {
        let view = st.public_view();
        let me = st.seat_of(player_id);
        // M5 (fix wave): `data-count` elsewhere on `#lc-hand` is the live
        // card count (`hand.len()`); a finished game has no hand to count,
        // and no JS reads this attribute — `lcApply`'s stale-drop keys off
        // `data-seq` alone (see the fn doc above). Hardcoded 0 to keep the
        // §7.8 DOM contract's attribute present rather than to feed a
        // consumer.
        // Review fix: data-pulls="0" so the tab row's pull count clears on
        // the finished-game repaint instead of freezing at its last
        // mid-game value.
        let pane = format!(
            r#"<div id="lc-hand" data-seq="{seq}" data-count="0" data-pulls="0">{card}</div>"#,
            seq = view.seq,
            card = lc_render::lc_end_card(&view, me),
        );
        let bar = lc_render::lc_action_bar(&action_bar_view(st, player_id));
        return format!(r#"{pane}<template data-lc-actions>{bar}</template>"#);
    }
    let seat = st.seat_of(player_id);
    let (hand, armed, locked, handicap_pct, pulls_left) = match seat {
        Some(seat) => {
            let p = &st.players[seat];
            let armed_cards = if p.locked {
                st.staged_for(seat).into_iter().cloned().collect()
            } else {
                p.armed.iter().map(|a| a.card.clone()).collect::<Vec<_>>()
            };
            // Pack 2: the tab row's pull count — the viewer's own pulls
            // left, summed over their vessels.
            let pulls: u16 = p.vessels.iter().map(|v| v.pulls_left as u16).sum();
            (
                p.hand.as_slice(),
                armed_cards,
                p.locked,
                p.handicap_pct,
                pulls,
            )
        }
        None => (&[] as &[_], Vec::new(), false, 100, 0),
    };
    let hg = HandGroupView {
        hand,
        armed: &armed,
        locked,
        handicap_pct,
        pulls_left,
        // I1 (Plan H review): the CostRail prices through the same
        // `cost_halved` seam `arm`/`lock_in`/the reveal charge/the DRINK
        // chip all agree on, so a Happy Hour rail bar can't disagree with
        // what the engine will actually charge.
        halved: st.cost_halved(),
    };
    // Plan J Task 4 / E1: the lobby is round-1 Draw with no outcome — the
    // early return above already ruled outcome out, so the gate collapses
    // to these two fields.
    let lobby = st.round == 1 && st.beat == Beat::Draw;
    let pane = lc_render::lc_hand_pane(base_path, code, &hg, st.seq, lobby);
    // Plan I Task 5: the response window (Alive) or the ghost note
    // (Eliminated) — an unseated spectator gets neither, same gating as the
    // tab panel below. A ghost's tabs were already voided at elimination
    // (H10); this is the reaction-window equivalent — they hold nothing to
    // answer with, but the table still hears their haunt (the action bar's
    // HAUNT row, not this section).
    let response = match seat {
        Some(s) if st.players[s].status == Status::Eliminated => {
            r#"<p class="lc-ghost-note">GHOST — YOU HOLD NOTHING. THE TABLE STILL HEARS YOU.</p>"#
                .to_string()
        }
        Some(s) => response_section_html(st, s),
        None => String::new(),
    };
    // Plan H Task 5 / H13: the private tab card, gated the same way the
    // action bar's per-viewer state is — seated and Alive only. An
    // Eliminated viewer's tabs were already voided at elimination (H10), and
    // the E7 "you're out" hint already tells them so; an unseated spectator
    // never held one to begin with.
    let tab_panel = match seat {
        Some(s) if st.players[s].status == Status::Alive => {
            let tab = st.players[s]
                .tabs
                .last()
                .and_then(|id| crate::lc_tabs::tab_def(id));
            lc_render::lc_tab_panel(tab)
        }
        _ => String::new(),
    };
    // Pack 2: the inspect sheet rides the same private fetch as the hand
    // it describes — skeleton + per-card stash, hidden until a wheel tap.
    // Review fix: PLAY only renders inside the staging window (the same
    // gate table_pane_html uses for the tray) — outside it the sheet says
    // when playing opens instead of dead-ending on a tray-less TABLE.
    let staging = matches!(st.beat, Beat::Diplomacy | Beat::Lock)
        && seat.is_some_and(|s| st.players[s].status == Status::Alive && !st.players[s].locked);
    let sheet = lc_render::lc_inspect_sheet(hand, staging);
    // Pack 3: the mulligan overlay — only while the engine would accept
    // the post (Draw beat, alive, holding a hand, round-1 lobby or the
    // round's swap unspent), so the MULLIGAN button and the overlay
    // appear and disappear together.
    let mull = match seat {
        Some(s)
            if st.beat == Beat::Draw
                && st.players[s].status == Status::Alive
                && !st.players[s].hand.is_empty()
                && (st.round == 1 || !st.players[s].mulliganed) =>
        {
            lc_render::lc_mulligan_overlay(&st.players[s].hand, st.round)
        }
        _ => String::new(),
    };
    // Challenge-cards container: the vote section rides the private hand
    // fetch like the response window — seated viewers only (a spectator
    // has no vote and the banner already carries the tally).
    let chal = match seat {
        Some(s) => challenge_section_html(st, s),
        None => String::new(),
    };
    let bar = lc_render::lc_action_bar(&action_bar_view(st, player_id));
    format!(
        r#"{pane}{response}{chal}{tab_panel}{sheet}{mull}<template data-lc-actions>{bar}</template>"#
    )
}

/// Plan E Task 4: assembles the viewer's own `ActionBarView` from
/// `&LastCallState` — the one place `LcPlayer`/`Play` fields are read down
/// into the private, per-viewer projection `lc_action_bar` renders from.
/// `charged` is the viewer's own pulls at the reveal (E9): summed over
/// `st.plays` (revealed, priced) filtered to their own seat, through their
/// own current `handicap_pct` — 0 for an unseated viewer, who has nothing
/// charged. `vessels_registered` counts every player (not just the viewer)
/// with at least one vessel — the E1 gate on starting round 1.
fn action_bar_view(st: &LastCallState, player_id: i64) -> ActionBarView {
    let outcome = st.outcome();
    let vessels_registered = st.players.iter().filter(|p| !p.vessels.is_empty()).count();
    match st.seat_of(player_id) {
        Some(seat) => {
            let p = &st.players[seat];
            // H12: `charged_pulls` is event-aware — the DRINK chip and the
            // reveal charge must always agree on the same number.
            //
            // I-1 (Plan I review, chip half): `charged_pulls` only sums
            // `st.plays` — a reaction's pulls (`play_reaction`, also
            // `effective_pull_cost`-priced) are deducted from the vessel at
            // play time but never entered this total, so the physical
            // prompt silently under-counted a seat that answered. `reactions`
            // is public the instant it's played (I9/TBD-7, same as `plays`
            // by reveal time), so summing it here leaks nothing new — add
            // it on top without touching `charged_pulls` itself, which
            // `SpentAtLeast`/`TopSpenderHit` also read (deliberately
            // untouched; parked for the user, per the review).
            let reaction_charged: u8 = st
                .reactions
                .iter()
                .filter(|r| r.source_seat == seat)
                .map(|r| st.effective_pull_cost(r.card.cost, p.handicap_pct))
                .fold(0u8, u8::saturating_add);
            let charged: u8 = st.charged_pulls(seat).saturating_add(reaction_charged);
            ActionBarView {
                beat: st.beat,
                round: st.round,
                seated: true,
                alive: p.status == Status::Alive,
                locked: p.locked,
                ready: p.ready,
                mulliganed: p.mulliganed,
                drawing: p.drawing,
                vessels: p
                    .vessels
                    .iter()
                    .enumerate()
                    .map(|(i, v)| (i, v.deck))
                    .collect(),
                charged,
                vessels_registered,
                outcome,
                haunt_plays: haunt_plays(st),
                haunted: st.haunts.iter().any(|h| h.seat == seat),
                armed_count: p.armed.len(),
            }
        }
        None => ActionBarView {
            beat: st.beat,
            round: st.round,
            seated: false,
            alive: false,
            locked: false,
            ready: false,
            mulliganed: false,
            drawing: false,
            vessels: Vec::new(),
            charged: 0,
            vessels_registered,
            outcome,
            haunt_plays: Vec::new(),
            haunted: false,
            armed_count: 0,
        },
    }
}

/// Plan I Task 5: every play currently in flight whose `card_fx` op is
/// `Damage` — DDv2 §9.2's "the only legal target is a Damage play" — as
/// `(order_key, "SRC → TGT")` captions (`play_caption` below). Read for
/// every seated viewer regardless of `alive`/`beat` — `lc_action_bar` only
/// reaches for this when the viewer is both a ghost and mid-Reveal, so an
/// unused value the rest of the time costs nothing worth special-casing
/// away.
fn haunt_plays(st: &LastCallState) -> Vec<(u32, String)> {
    st.plays
        .iter()
        .filter(|p| {
            matches!(
                crate::lc_cards::card_fx(&p.card.id).map(|f| f.op),
                Some(EffectOp::Damage)
            )
        })
        .map(|p| (p.order_key, play_caption(st, p)))
        .collect()
}

/// One play's "SRC → TGT" caption — the route-side twin of `lc_render`'s
/// private `centre_play`/E15 convention (that builder reads `&PublicView`
/// only, per §3.4, so it cannot be reused here where the caller still holds
/// `&LastCallState`): the source seat's name, then either the target
/// seat's name or the card's own `targets` field uppercased ("ALL") when
/// the play has none. Shared by `response_section_html`'s button captions
/// and `haunt_plays`' above.
fn play_caption(st: &LastCallState, play: &Play) -> String {
    let src = seat_name(st, play.source_seat);
    let tgt = match play.target {
        Some(t) => seat_name(st, t),
        None => crate::render::html_escape(&play.card.targets.to_uppercase()),
    };
    format!("{src} → {tgt}")
}

// Pack 1 (lc-mobile-play-flow) retired `targets_section_html` — the E8
// per-card `<select>` target picker. Targeting now happens in the TABLE
// tab's full-pane overlay at arm time (`lc_target_overlay` + the
// `lc_loop.js` tray/overlay wiring, which posts `arm` then `target`); the
// `/lastcall/target` route itself is unchanged.

/// The route-side twin of `last_call::play_subjects` (private to that
/// module, and not in this task's file list to touch): the answered play's
/// subject seats, by its own `card.targets` — `"one"` is the play's single
/// `target` if it has one, `"self"` is just the caster, `"all"` is every
/// seat, anything else names nobody. Used only by `scope_legal` below, to
/// decide whether a `targets == "self"` reaction may answer this play (I5).
fn play_subjects(play: &Play, num_seats: usize) -> Vec<usize> {
    match play.card.targets.as_str() {
        "one" => play.target.into_iter().collect(),
        "self" => vec![play.source_seat],
        "all" => (0..num_seats).collect(),
        _ => Vec::new(),
    }
}

/// I5's scope filter, read for display the same way `play_reaction` reads
/// it for real: a `targets == "self"` reaction card may only answer a play
/// whose subject set includes the reactor's own seat; regardless of scope,
/// `Reflect` needs a single seat to send the play home to, so it refuses an
/// untargeted (`None`) play. Cost/afford is deliberately not checked here —
/// the brief's "scope-legal" is a legality question, not an affordability
/// one, and `play_reaction` itself is still the sole authority either way
/// (this is display-only; a 409 still guards the real submit).
fn scope_legal(
    seat: usize,
    num_seats: usize,
    card: &Card,
    rfx: Option<crate::lc_cards::ReactionFx>,
    play: &Play,
) -> bool {
    if card.targets == "self" && !play_subjects(play, num_seats).contains(&seat) {
        return false;
    }
    // Mirror of play_reaction's Reflect guard (review wave): a challenge
    // play is judged by its contest — a Duel takes the role swap, a Solo
    // has no roles to swap and offering SEND BACK would sell a dead spend
    // — while a numeric play keeps the original single-seat requirement.
    if matches!(rfx, Some(crate::lc_cards::ReactionFx::Reflect)) {
        match crate::lc_cards::card_chfx(&play.card.id) {
            Some(c) if c.contest == crate::lc_cards::Contest::Duel => {}
            Some(_) => return false,
            None if play.target.is_none() => return false,
            None => {}
        }
    }
    true
}

/// Plan I Task 5 / decision I12: the hand-pane response window, `hand_pane_
/// html`'s private per-viewer analogue of `targets_section_html` above.
/// Empty string unless `beat == Beat::Reveal`, the viewer is `Alive`, and
/// scope-legality (`scope_legal`) admits at least one play for at least one
/// reaction card in hand — otherwise the window's very presence would leak
/// who is holding what (I2 closes this the same way for the route: the
/// window opens unconditionally, every round, full duration). One
/// `.lc-react-card` block per reaction card the viewer holds, each with one
/// button per scope-legal play, in `order_key` order.
fn response_section_html(st: &LastCallState, seat: usize) -> String {
    if st.beat != Beat::Reveal || st.players[seat].status != Status::Alive {
        return String::new();
    }
    let num_seats = st.players.len();
    let mut plays: Vec<&Play> = st.plays.iter().collect();
    plays.sort_by_key(|p| p.order_key);

    let mut any_legal = false;
    let blocks: String = st.players[seat]
        .hand
        .iter()
        .filter(|c| c.kind == CardKind::Reaction)
        .map(|card| {
            let rfx = crate::lc_cards::card_rfx(&card.id);
            let verb = match rfx {
                Some(crate::lc_cards::ReactionFx::Cancel) => "CANCEL",
                Some(crate::lc_cards::ReactionFx::Reduce(_)) => "BLUNT",
                Some(crate::lc_cards::ReactionFx::Reflect) => "SEND BACK",
                None => "", // rfx is Some ⇔ kind == Reaction (F5) — defensive only
            };
            let buttons: String = plays
                .iter()
                .filter(|p| scope_legal(seat, num_seats, card, rfx, p))
                .map(|p| {
                    any_legal = true;
                    format!(
                        r#"<button class="lc-btn lc-react-btn" data-lc-post="react" data-card-id="{id}" data-play="{k}">{verb} {caption}</button>"#,
                        id = crate::render::html_escape(&card.id),
                        k = p.order_key,
                        caption = play_caption(st, p),
                    )
                })
                .collect();
            format!(
                r#"<div class="lc-react-card" data-card-id="{id}"><span class="lc-react-title">{title}</span>{buttons}</div>"#,
                id = crate::render::html_escape(&card.id),
                title = crate::render::html_escape(&card.title),
            )
        })
        .collect();
    if !any_legal {
        return String::new();
    }
    format!(r#"<section class="lc-react"><h2>Response window</h2>{blocks}</section>"#)
}

/// Challenge-cards container (Pack 1, bare loop): the hand pane's vote
/// section while the game is parked. Eligible viewers get the two verdict
/// buttons (`data-lc-post` + pre-encoded `data-lc-body`, the pact-button
/// pattern); contestants, ghosts and already-voted seats get status copy.
/// Renders nothing when no challenge is active.
fn challenge_section_html(st: &LastCallState, seat: usize) -> String {
    let Some(ch) = st.challenges.first() else {
        return String::new();
    };
    let card = crate::lc_cards::card_by_id(&ch.card_id);
    let title = card
        .as_ref()
        .map(|c| crate::render::html_escape(&c.title))
        .unwrap_or_else(|| crate::render::html_escape(&ch.card_id));
    let text = card
        .as_ref()
        .map(|c| crate::render::html_escape(&c.text))
        .unwrap_or_default();
    let head = match ch.opponent {
        Some(o) => format!("{} VS {}", seat_name(st, ch.instigator), seat_name(st, o)),
        None => format!("{} PERFORMS", seat_name(st, ch.instigator)),
    };
    let body = if seat == ch.instigator || Some(seat) == ch.opponent {
        r#"<p class="lc-chal-note">THE TABLE IS DECIDING. STATE YOUR CASE.</p>"#.to_string()
    } else if st.players[seat].status != Status::Alive {
        r#"<p class="lc-chal-note">GHOSTS WATCH IN SILENCE.</p>"#.to_string()
    } else if ch.votes.iter().any(|v| v.voter == seat) {
        r#"<p class="lc-chal-note">VOTE CAST. WAITING ON THE TABLE.</p>"#.to_string()
    } else if !ch.electorate.contains(&seat) {
        // Seated after the electorate froze at activation (review wave).
        r#"<p class="lc-chal-note">YOU ARRIVED MID-ARGUMENT. WATCH THIS ONE.</p>"#.to_string()
    } else {
        let (for_label, against_label) = match ch.opponent {
            Some(o) => (
                format!("{} WINS", seat_name(st, ch.instigator)),
                format!("{} WINS", seat_name(st, o)),
            ),
            None => (
                "IMPRESSED — PASS".to_string(),
                "NOT IMPRESSED — FAIL".to_string(),
            ),
        };
        format!(
            r#"<button class="lc-btn lc-chal-btn" data-lc-post="challenge-vote" data-lc-body="challenge={key}&amp;for_instigator=true">{for_label}</button><button class="lc-btn lc-chal-btn" data-lc-post="challenge-vote" data-lc-body="challenge={key}&amp;for_instigator=false">{against_label}</button>"#,
            key = ch.key,
        )
    };
    format!(
        r#"<section class="lc-chal" data-lc-chal><h2>CHALLENGE — {head}</h2><p class="lc-chal-title">{title}</p><p class="lc-chal-text">{text}</p>{body}</section>"#
    )
}

/// A seat's name, uppercased and escaped — the `&LastCallState` analogue
/// of `lc_render::seat_name_upper`, which reads `&PublicView` and so cannot
/// be reused here. `.get()`, not `[]`, for the same defensive reason
/// `seat_name_upper` gives — a stored seat index outliving the player it
/// named is a corrupt-blob concern, not a panic.
fn seat_name(st: &LastCallState, seat: usize) -> String {
    st.players
        .get(seat)
        .map(|p| crate::render::html_escape(&p.name.to_uppercase()))
        .unwrap_or_default()
}

// NOTE: the brief's Produces section lists a `seq: u64` field on this struct,
// but its own literal `lc_room.html` markup never consumes it (`#lc-hand`,
// embedded inside `hand_pane`, already carries the §7.8-required `data-seq`).
// An unused field is a hard `dead_code` warning under this crate's
// zero-warnings gate, and the two ways to silence it disagree with each
// other: adding a second `data-seq` (e.g. on `<body>`) would leave two
// `[data-seq]` nodes in one document, which breaks a naive
// `document.querySelector("[data-seq]")` in Task 3's SSE client by document
// order. Dropping the unused field has no observable effect today, so that's
// the resolution here — flagged for whoever writes Task 3's reconnect
// tracking to decide where the page-level seq should live.
#[derive(Template)]
#[template(path = "lc_room.html")]
struct LcRoomTemplate {
    base_path: String,
    code: String,
    player_id: i64,
    /// Test play mode's identity switcher, or `""` (the flag-off default).
    test_bar: String,
    banner: String,     // lc_render::lc_banner(&view)
    hand_pane: String,  // lc_render::lc_hand_pane(...)
    table_pane: String, // table_pane_html(&view, me) — the #lc-table fragment
    actions: String,    // lc_render::lc_action_bar(&action_bar_view(&ctx.st, player.id))
    log_pane: String,   // lc_render::lc_log(&view)
}

/// `GET /room/{code}/lastcall` — the F.1 phone shell. `load_lc` already gates
/// member -> active game -> kind, so a non-member gets 403 and a Ring of Fire
/// room gets `WrongGameKind` for free. A logged-in member who is somehow not
/// seated (a race with the late-join hook in `routes.rs::room_page`) gets an
/// empty hand rather than an error.
pub async fn lc_page(
    State(state): State<GameState>,
    PlayerSession(player): PlayerSession,
    Path(code): Path<String>,
) -> axum::response::Response {
    let ctx = match load_lc(&state, &code, &player).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let hand_pane = hand_pane_html(&state.base_path, &code, &ctx.st, player.id);
    let actions = lc_render::lc_action_bar(&action_bar_view(&ctx.st, player.id));
    let view = ctx.st.public_view();
    let test_bar = if state.test_mode {
        let members = db::room_members(&state.pool, ctx.room.id).await;
        crate::render::test_switcher_bar(&state.base_path, &code, &members, player.id)
    } else {
        String::new()
    };
    let tpl = LcRoomTemplate {
        base_path: state.base_path.to_string(),
        code,
        player_id: player.id,
        test_bar,
        banner: lc_render::lc_banner(&view),
        hand_pane,
        table_pane: table_pane_html(&ctx.st, &view, player.id),
        actions,
        log_pane: lc_render::lc_log(&view),
    };
    Html(tpl.render().unwrap()).into_response()
}

/// The `#lc-table` fragment: the F.3 mini table (`lc_render::lc_mini_table`)
/// wrapped with the `data-seq` freshness marker, mirroring `lc_hand_pane`'s
/// `#lc-hand` root. Shared by `lc_page` (initial paint) and
/// `lc_table_handler` (the per-viewer refetch) so the two can never
/// disagree on the fragment's shape for the same state.
///
/// Pack 1 (lc-mobile-play-flow): now takes `&LastCallState` + `player_id`
/// (the `hand_pane_html` shape) rather than `&PublicView` + `me` — the
/// TABLE fetch was already per-viewer and session-gated, and the play
/// surface adds the viewer's OWN tray, targeting overlay and armed stack
/// to it. Those three render only for a seated, ALIVE viewer during the
/// staging beat (Diplomacy; Lock for a legacy blob) with no outcome — the
/// same window `arm`/`disarm` accept — and read only the viewer's own
/// seat, so player A's fragment can never carry player B's cards. A
/// locked viewer keeps the stack (their own `locked_plays`, `data-locked`,
/// no take-back) but loses the tray and overlay: the queue is committed.
/// Review fix: takes the already-built `&PublicView` alongside the state
/// instead of projecting its own copy — `lc_page` builds one view for the
/// banner/log and passes it here, so a page render projects once, not
/// twice. Callers must pass a view built from the SAME `st`.
fn table_pane_html(st: &LastCallState, view: &PublicView, player_id: i64) -> String {
    let me = st.seat_of(player_id);
    let staging = matches!(st.beat, Beat::Diplomacy | Beat::Lock) && st.outcome().is_none();
    let mut stack = String::new();
    let mut tray = String::new();
    let mut overlay = String::new();
    if let Some(seat) = me {
        let p = &st.players[seat];
        if staging && p.status == Status::Alive {
            if p.locked {
                let staged: Vec<(&Card, Option<usize>)> = st
                    .locked_plays
                    .iter()
                    .filter(|pl| pl.source_seat == seat)
                    .map(|pl| (&pl.card, pl.target))
                    .collect();
                stack = lc_render::lc_table_stack(&staged, true, view, seat);
            } else {
                let armed: Vec<(&Card, Option<usize>)> =
                    p.armed.iter().map(|a| (&a.card, a.target)).collect();
                stack = lc_render::lc_table_stack(&armed, false, view, seat);
                tray = lc_render::lc_tray(&p.hand);
                overlay = lc_render::lc_target_overlay(view, seat);
            }
        }
    }
    format!(
        r#"<div id="lc-table" data-seq="{seq}"><div class="lc-tablescene" data-lc-scene-table>{mini}{stack}<svg class="lc-arrowlay" data-lc-arrows aria-hidden="true"></svg></div>{tray}{overlay}</div>"#,
        seq = view.seq,
        mini = lc_render::lc_mini_table(view, me),
    )
}

/// `GET /room/{code}/lastcall/table` — PER VIEWER.
///
/// The mini table's underlying data is entirely public — it's the same
/// `PublicView` the big screen renders from `LcPublic` — but the LAYOUT is
/// not: D.2 rotates the ring so the viewer's own seat sits at
/// bottom-centre, and no two players share a rotation. A `RoomHub`
/// broadcast is one fragment for the whole room and cannot carry a
/// per-viewer rotation, so this is fetched rather than pushed — same reason
/// `lc_hand_handler` below is a fetch, not a broadcast.
///
/// Takes no player identifier of any kind: no path segment, no query
/// parameter, no form field. The viewer's identity comes from the session
/// cookie alone, via `PlayerSession`. Written this way, "can player A fetch
/// player B's rotation?" is unanswerable rather than merely guarded, and a
/// reviewer can verify it from this signature — the same property
/// `lc_hand_handler` establishes for hands (spec §6.1). A room member who
/// has not been seated (joined mid-game, no vessel yet) passes `None` to
/// `lc_mini_table` and gets the unrotated table, the same branch `lc_page`
/// already takes for the hand.
pub async fn lc_table_handler(
    State(state): State<GameState>,
    PlayerSession(player): PlayerSession,
    Path(code): Path<String>,
) -> axum::response::Response {
    let ctx = match load_lc(&state, &code, &player).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let view = ctx.st.public_view();
    Html(table_pane_html(&ctx.st, &view, player.id)).into_response()
}

/// `GET /room/{code}/lastcall/hand` — PRIVATE.
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
) -> axum::response::Response {
    let ctx = match load_lc(&state, &code, &player).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    Html(hand_pane_html(&state.base_path, &code, &ctx.st, player.id)).into_response()
}

// -------------------------------------------------------------
// Plan E (Task 1): the beat-loop action routes — arm, disarm, target, lock,
// draw. All five share `lc_lock` -> `load_lc` for the member/active-game/kind
// gate, mutate `ctx.st` in place, then persist. arm/disarm/set_target
// publish only `LcTick` (decision E5: nothing they change is legible on any
// public surface, so a full re-render/re-broadcast would carry no public
// information — but every phone still needs to know to re-fetch its own
// private fragment). lock and draw publish the full set via
// `persist_and_broadcast_lc`, because both move something public: the lock
// tick (`PublicSeat::locked`) and, for draw, the deck counts.
// -------------------------------------------------------------

/// Engine error -> HTTP. NotSeated/NotAlive/NotAGhost are "you have no say
/// here" (403, like tm's NotYourCall — NotAGhost is haunt's mirror of
/// NotAlive: a live seat has no vote to cast, I10); WrongBeat/AlreadyLocked/
/// MustResolve/AlreadyHaunted are "not now" (409, like tm's OutOfTurn —
/// AlreadyHaunted is the once-per-round guard, 9.2, and WrongBeat is the
/// react/haunt window's own transport face: a response after Reveal has
/// closed is "not now", not "never you", decision I3); the two named-card
/// refusals carry their message as a plain-text 422 body the action bar
/// shows verbatim (DDv2 6.3 "naming the card"); everything else
/// (UnknownCard, NotPlayable, BadTarget, BadDraw) is a bare 422. `lock_in`
/// replay after a beat tick has already moved past `Beat::Lock` returns
/// `WrongBeat`, not the idempotent `Ok(())` lock_in gives a same-beat
/// replay — that's still "not now" from the caller's side, so it takes the
/// same 409 as every other WrongBeat case rather than a special-cased
/// mapping.
pub(crate) fn map_lc(e: LcError) -> axum::response::Response {
    match e {
        LcError::NotSeated | LcError::NotAlive | LcError::NotAGhost => {
            GameError::NotYourCall.into_response()
        }
        LcError::WrongBeat
        | LcError::AlreadyLocked
        | LcError::MustResolve
        | LcError::AlreadyHaunted
        | LcError::ChallengePending
        | LcError::CantVote => GameError::OutOfTurn.into_response(),
        LcError::CantAfford(id) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            format!("Can't afford {id}."),
        )
            .into_response(),
        LcError::NeedsTarget(id) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            format!("{id} needs a target."),
        )
            .into_response(),
        LcError::PactBlocked => {
            (StatusCode::UNPROCESSABLE_ENTITY, "No pact to be had.").into_response()
        }
        LcError::NoOffer => {
            (StatusCode::UNPROCESSABLE_ENTITY, "That offer is gone.").into_response()
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

#[derive(Deserialize)]
pub struct CardForm {
    pub card_id: String,
}

/// `POST /room/{code}/lastcall/arm` — private (`LcTick` only, decision E5).
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

/// `POST /room/{code}/lastcall/disarm` — private (`LcTick` only, decision
/// E5).
pub async fn lc_disarm_handler(
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
    if let Err(e) = ctx.st.disarm(player.id, &form.card_id) {
        return map_lc(e);
    }
    persist_and_tick_lc(&state, &ctx).await;
    StatusCode::NO_CONTENT.into_response()
}

#[derive(Deserialize)]
pub struct LcTargetForm {
    pub card_id: String,
    #[serde(default)]
    pub target: String,
}

/// `POST /room/{code}/lastcall/target` — private (`LcTick` only, decision
/// E5). `target=""` means "no target" (self/all/table cards, and clearing a
/// one-target card back to unset); any other value must parse as a seat
/// index or the request is a bare 422 before the engine ever sees it.
pub async fn lc_target_handler(
    State(state): State<GameState>,
    PlayerSession(player): PlayerSession,
    Path(code): Path<String>,
    Form(form): Form<LcTargetForm>,
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
    let target = if form.target.is_empty() {
        None
    } else {
        match form.target.parse::<usize>() {
            Ok(n) => Some(n),
            Err(_) => return StatusCode::UNPROCESSABLE_ENTITY.into_response(),
        }
    };
    if let Err(e) = ctx.st.set_target(player.id, &form.card_id, target) {
        return map_lc(e);
    }
    persist_and_tick_lc(&state, &ctx).await;
    StatusCode::NO_CONTENT.into_response()
}

/// `POST /room/{code}/lastcall/lock` — public: the lock tick
/// (`PublicSeat::locked`) is legible on the mini table and the big screen,
/// so this rides `persist_and_broadcast_lc` rather than the tick-only path.
/// Task 2 adds the all-locked early advance into this handler.
pub async fn lc_lock_handler(
    State(state): State<GameState>,
    PlayerSession(player): PlayerSession,
    Path(code): Path<String>,
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
    if let Err(e) = ctx.st.lock_in(player.id) {
        return map_lc(e);
    }
    // Decision E3, now Diplomacy's ONLY exit (clock removal + beat
    // restructure, 2026-08-13): the last alive seat locking is what flips
    // the table. `Beat::Lock` accepted for a legacy blob parked there.
    if matches!(ctx.st.beat, Beat::Diplomacy | Beat::Lock)
        && ctx
            .st
            .players
            .iter()
            .filter(|p| p.status == Status::Alive)
            .all(|p| p.locked)
    {
        lc_advance_chain(&mut ctx.st); // Lock -> Reveal
    }
    persist_and_broadcast_lc(&state, &ctx).await;
    StatusCode::NO_CONTENT.into_response()
}

/// `POST /room/{code}/lastcall/ready` — the open beats' advance, and since
/// the clock's removal (2026-08-13) the only thing that moves Draw (round
/// ≥ 2), Diplomacy and Reveal. `lc_lock_handler`'s exact shape: engine
/// mutation, then the all-ready early advance under the same guard, then
/// the full public broadcast (the ready tick is legible on the mini table
/// and the big screen, like the lock tick).
pub async fn lc_ready_handler(
    State(state): State<GameState>,
    PlayerSession(player): PlayerSession,
    Path(code): Path<String>,
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
    if let Err(e) = ctx.st.set_ready(player.id) {
        return map_lc(e);
    }
    // set_ready succeeding proves the beat is an open one, so the only
    // predicate left is the table's: everyone alive ready -> advance.
    if ctx.st.all_ready() {
        lc_advance_chain(&mut ctx.st);
    }
    persist_and_broadcast_lc(&state, &ctx).await;
    StatusCode::NO_CONTENT.into_response()
}

#[derive(Deserialize)]
pub struct DrawForm {
    pub vessel: usize,
}

/// `POST /room/{code}/lastcall/draw` — public: the shoe's deck count and the
/// drawing pulse are both legible on the mini table / big screen. The one
/// route with RNG: card identity is decided HERE (D6), never in the engine
/// — `finish_and_draw` only validates that what the caller sampled matches
/// the vessel's deck and the expected count. Pre-reads under the guard
/// before touching the RNG: `seat_of` (else `NotYourCall`, this route's own
/// gate since `finish_and_draw`'s `NotSeated` case is unreachable once
/// `seat_of` has already succeeded), then the vessel at `form.vessel` (else
/// a bare 422 — an out-of-range vessel index is a malformed request, not a
/// "not now"), then that vessel's deck's shoe count. Samples from
/// `lc_cards::shoe(deck)` — the real copy-weighted 40-card shoe (Plan F),
/// not `deck_cards` — so higher-copy cards are proportionally more likely,
/// matching the composition `deck_counts` is tracking down. Duplicates
/// within one draw are expected (F11: shoe sampling is with replacement;
/// nothing here removes a card once drawn).
pub async fn lc_draw_handler(
    State(state): State<GameState>,
    PlayerSession(player): PlayerSession,
    Path(code): Path<String>,
    Form(form): Form<DrawForm>,
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
    let Some(seat) = ctx.st.seat_of(player.id) else {
        return GameError::NotYourCall.into_response();
    };
    let Some(vessel) = ctx.st.players[seat].vessels.get(form.vessel) else {
        return StatusCode::UNPROCESSABLE_ENTITY.into_response();
    };
    let deck = vessel.deck;
    let shoe_count = ctx
        .st
        .deck_counts
        .iter()
        .find(|(d, _)| *d == deck)
        .map(|&(_, c)| c)
        .unwrap_or(0);
    let need = DRAW_PER_VESSEL.min(shoe_count as usize);
    let pool_cards = crate::lc_cards::shoe(deck);
    let drawn: Vec<Card> = {
        let mut rng = rand::thread_rng();
        (0..need)
            .map(|_| pool_cards[rng.gen_range(0..pool_cards.len())].clone())
            .collect()
    };
    if let Err(e) = ctx.st.finish_and_draw(player.id, form.vessel, drawn) {
        return map_lc(e);
    }
    persist_and_broadcast_lc(&state, &ctx).await;
    StatusCode::NO_CONTENT.into_response()
}

#[derive(Deserialize)]
pub struct MulliganForm {
    /// Comma-separated card ids to discard (duplicates allowed — a hand can
    /// hold two copies; each occurrence claims a distinct hand card).
    pub cards: String,
}

/// `POST /room/{code}/lastcall/mulligan` — the per-card discard/redraw
/// (beat-restructure, 2026-08-13). `lc_draw_handler`'s D6 split, one card
/// at a time: replacements are shoe-sampled HERE, each from its discarded
/// card's own deck, and `st.mulligan` validates everything (beat, the
/// round-1-unlimited/one-per-round-after rule, id/deck/shoe coherence).
/// Public shape changes (deck counts, discard count, the log line) ride the
/// full broadcast, same as draw.
pub async fn lc_mulligan_handler(
    State(state): State<GameState>,
    PlayerSession(player): PlayerSession,
    Path(code): Path<String>,
    Form(form): Form<MulliganForm>,
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
    let Some(seat) = ctx.st.seat_of(player.id) else {
        return GameError::NotYourCall.into_response();
    };
    let ids: Vec<String> = form
        .cards
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect();
    if ids.is_empty() {
        return StatusCode::UNPROCESSABLE_ENTITY.into_response();
    }
    // Resolve each id to a distinct hand card to learn its deck — the same
    // claim walk `st.mulligan` re-runs as the authority; an id that doesn't
    // resolve here gets the engine's own `UnknownCard` shortly anyway.
    let hand = &ctx.st.players[seat].hand;
    let mut taken: Vec<usize> = Vec::with_capacity(ids.len());
    for id in &ids {
        let Some(idx) = hand
            .iter()
            .enumerate()
            .find(|(i, c)| c.id == *id && !taken.contains(i))
            .map(|(i, _)| i)
        else {
            return map_lc(crate::last_call::LcError::UnknownCard);
        };
        taken.push(idx);
    }
    let replacements: Vec<crate::last_call::Card> = {
        let mut rng = rand::thread_rng();
        taken
            .iter()
            .map(|&i| {
                let pool = crate::lc_cards::shoe(hand[i].deck);
                pool[rng.gen_range(0..pool.len())].clone()
            })
            .collect()
    };
    if let Err(e) = ctx.st.mulligan(player.id, &ids, replacements) {
        return map_lc(e);
    }
    persist_and_broadcast_lc(&state, &ctx).await;
    StatusCode::NO_CONTENT.into_response()
}

// -------------------------------------------------------------
// Plan G (Task 4): the pact routes — offer/accept/decline. Screen-declutter
// pack (2026-08-13): the phone's pact section no longer renders, so no
// button posts here anymore — the routes and the engine stay, dormant, for
// a future pact redesign. All three share
// Plan E Task 1's exact skeleton (`lc_lock` -> `load_lc` -> mutate -> `map_lc`
// on error -> persist -> `204`) and, like arm/disarm/target, publish
// `LcTick` alone (tick-only — E5's rule applied a second time): nothing
// `offer_pact`/`accept_pact`/`decline_pact` ever changes is legible on any
// public surface — `pacts`/`pact_offers`/`pact_barred` are never projected
// by `public_view()` (G13), so a full re-render/re-broadcast would carry no
// public information at all, only free the market's private state to a
// spectator who has no business seeing a market exists. But both parties'
// own phones still need the private re-fetch signal to repaint their own
// hand fragment, and the spectator screen never notices: it consumes
// only `LcPublic`, so it has no listener for `LcTick` to ignore in the
// first place ("who is subscribed and what are they looking at").
//
// None of the three takes a player identifier for the *offering* viewer:
// that comes from the session cookie alone via `PlayerSession`, the same
// spec §6.1 constraint-not-check shape `lc_hand_handler`/`lc_table_handler`
// establish for private fragments — "can player A act as player B?" is
// unanswerable rather than merely guarded. `target`/`from` name the OTHER
// seat only, which is exactly what the brief's `PactOfferForm`/
// `PactFromForm` carry.
// -------------------------------------------------------------

#[derive(Deserialize)]
pub struct PactOfferForm {
    pub target: usize,
}

#[derive(Deserialize)]
pub struct PactFromForm {
    pub from: usize,
}

/// `POST /room/{code}/lastcall/pact/offer` — private (`LcTick` only, see the
/// section comment above).
pub async fn lc_pact_offer_handler(
    State(state): State<GameState>,
    PlayerSession(player): PlayerSession,
    Path(code): Path<String>,
    Form(form): Form<PactOfferForm>,
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
    if let Err(e) = ctx.st.offer_pact(player.id, form.target) {
        return map_lc(e);
    }
    persist_and_tick_lc(&state, &ctx).await;
    StatusCode::NO_CONTENT.into_response()
}

/// `POST /room/{code}/lastcall/pact/accept` — private (`LcTick` only, see the
/// section comment above).
pub async fn lc_pact_accept_handler(
    State(state): State<GameState>,
    PlayerSession(player): PlayerSession,
    Path(code): Path<String>,
    Form(form): Form<PactFromForm>,
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
    if let Err(e) = ctx.st.accept_pact(player.id, form.from) {
        return map_lc(e);
    }
    persist_and_tick_lc(&state, &ctx).await;
    StatusCode::NO_CONTENT.into_response()
}

/// `POST /room/{code}/lastcall/pact/decline` — private (`LcTick` only, see
/// the section comment above).
pub async fn lc_pact_decline_handler(
    State(state): State<GameState>,
    PlayerSession(player): PlayerSession,
    Path(code): Path<String>,
    Form(form): Form<PactFromForm>,
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
    if let Err(e) = ctx.st.decline_pact(player.id, form.from) {
        return map_lc(e);
    }
    persist_and_tick_lc(&state, &ctx).await;
    StatusCode::NO_CONTENT.into_response()
}

// -------------------------------------------------------------
// Plan E (Task 2), post-clock-removal (2026-08-13): the auto-beat advance
// chain, the migration ticker, and the begin route. The beat clock is gone
// — no route arms `beat_deadline_ms` any more; beats advance on the
// table's own taps (ready/lock). The field survives as DATA the ticker
// sweeps to `None` exactly once for an in-flight blob the previous binary
// persisted mid-countdown — still written and read only here (and in
// `mechanics::tick`, which just calls through to `lc_tick_room`); the
// engine (`last_call.rs`) never calls a clock function.
// -------------------------------------------------------------

/// Unix ms — the ticker's expiry check for stale pre-removal deadlines.
pub(crate) fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock before 1970")
        .as_millis() as i64
}

/// One user-visible advance plus every auto beat behind it (decision E4):
/// advance (or resolve, at Resolve), then chain through Deal and Resolve
/// until a player-driven beat or a game-over freeze. The beat clock is GONE
/// (2026-08-13): no deadline is ever armed — beats wait for the table (the
/// ready route's all-ready advance, the lock route's all-locked one) — and
/// the unconditional `None` at the end is the migration sweep for an
/// in-flight blob persisted with a deadline by the previous binary. The
/// expects are structural: advance_beat only fails at Resolve, which both
/// branches route to resolve(); resolve only fails off Resolve.
///
/// M3 mirror (review I1): `resolve()` deliberately no-ops on an empty
/// `players` — `Ok(())`, `beat` untouched, `seq` not bumped (last_call.rs's
/// own M3 hardening) — and `outcome()` is `None` below two players. Without
/// this early return, empty players at `Beat::Resolve` would loop forever
/// with no `.await` inside it: `outcome()` stays `None`, `beat` stays
/// `Resolve`, `resolve()` keeps no-oping, next iteration is identical. No
/// engine transition can produce an empty `players` (`from_json` truncates
/// to `MAX_SEATS`, never to zero), but a hand-corrupted or pre-ceiling blob
/// could, and the ticker calls this with no other bound on the loop — a spin
/// here pegs a core, never yields, and never releases the room mutex, which
/// is strictly worse than the panic this same case is elsewhere defended
/// against with `expect`/bounds checks.
pub(crate) fn lc_advance_chain(st: &mut LastCallState) {
    if st.players.is_empty() {
        return; // M3: nothing to advance; resolve() no-ops here and would spin
    }
    if st.challenge_pending() {
        // Parked in the challenge phase (challenge-cards container): the
        // vote flow owns the rollover; entering resolve() here would panic
        // its expect on ChallengePending.
        return;
    }
    if st.beat == Beat::Resolve {
        st.resolve()
            .expect("resolve() at Beat::Resolve cannot fail");
    } else {
        st.advance_beat()
            .expect("advance_beat() off Resolve cannot fail");
    }
    loop {
        if st.outcome().is_some() {
            st.beat_deadline_ms = None; // frozen final tableau (D16)
            return;
        }
        if st.challenge_pending() {
            // resolve() just parked the round — same freeze shape as the
            // outcome gate above; without this the Resolve arm below would
            // re-enter resolve() forever.
            st.beat_deadline_ms = None;
            return;
        }
        match st.beat {
            Beat::Deal => st
                .advance_beat()
                .expect("advance_beat() at Deal cannot fail"),
            Beat::Resolve => st
                .resolve()
                .expect("resolve() at Beat::Resolve cannot fail"),
            _ => break,
        }
    }
    st.beat_deadline_ms = None;
}

/// The stale-deadline migration sweep, ridden on mechanics.rs's global 1 Hz
/// ticker (decision E16). Since the clock's removal (2026-08-13) no route
/// arms a deadline, so on current blobs the advisory pre-check below is a
/// permanent early return; a blob persisted mid-countdown by the previous
/// binary gets its one last advance here (the chain then clears the field
/// for good). Advisory pre-check WITHOUT the lock first — one indexed
/// SELECT per hub-active room per second, almost always returning early —
/// then, only when a deadline has expired: take the room guard, RE-LOAD and
/// RE-CHECK under it (an action route may have advanced the beat between the
/// advisory read and the lock), run the chain, and persist_and_broadcast_lc
/// while the guard is still held. The re-check is what makes the ticker and
/// the lock route's early advance commute instead of double-advancing: both
/// compare the freshly-reloaded `beat_deadline_ms` against "now", not the
/// stale value seen before the lock, so whichever side gets the guard first
/// clears or re-arms the deadline and the loser's re-check sees the new
/// value and no-ops. `outcome().is_some()` mirrors `lc_advance_chain`'s own
/// freeze so a finished game's `None` deadline is never treated as
/// "expired" by the `is_none_or` below.
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
    lc_advance_chain(&mut st);
    let ctx = LcCtx { room, game, st };
    persist_and_broadcast_lc(state, &ctx).await;
}

/// `POST /room/{code}/lastcall/begin` — starts round 1's loop. Any
/// member may press it, the same `tm_roll_handler` any-member precedent (no
/// notion of "whose turn to begin" exists at the registration lobby). Refuses
/// off round 1's Draw (already begun, or — defensively — a state this route
/// should never see off the lobby) and refuses under two registered players
/// (`vessels.is_empty()` is "hasn't set a drink yet", the same test
/// `lc_start_handler`'s member-count gate uses one level up, but here it's
/// "registered", not merely "seated" — a member can join the room and sit
/// without ever calling `/vessel`). On success: Draw -> Deal (auto) ->
/// Diplomacy, untimed — the table's all-ready taps move it from here.
pub async fn lc_begin_handler(
    State(state): State<GameState>,
    PlayerSession(player): PlayerSession,
    Path(code): Path<String>,
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
    if ctx.st.round != 1 || ctx.st.beat != Beat::Draw {
        return GameError::OutOfTurn.into_response();
    }
    if ctx
        .st
        .players
        .iter()
        .filter(|p| !p.vessels.is_empty())
        .count()
        < 2
    {
        return GameError::TooFewPlayers.into_response();
    }
    lc_advance_chain(&mut ctx.st);
    persist_and_broadcast_lc(&state, &ctx).await;
    StatusCode::NO_CONTENT.into_response()
}

// -------------------------------------------------------------
// Plan I (Task 4): the react and haunt routes — the Reveal beat's response
// window. Both share Plan E Task 1's exact skeleton (`lc_lock` -> `load_lc`
// -> mutate -> `map_lc` on error -> persist -> 204) and, unlike arm/disarm/
// target/pact, publish the FULL set via `persist_and_broadcast_lc`: a played
// reaction and a cast haunt vote are both public the instant they land
// (I9/I10) — the chips and the hand count are legible on public surfaces
// once played, so "who is subscribed and what are they looking at" answers
// the same way for both, same as lock/draw.
//
// Decision I3 (the response window's grace extension) died with the beat
// clock (2026-08-13): Reveal now waits for the table's all-ready taps, so
// there is no deadline to extend and a response can never be "almost too
// late" — the window is exactly as long as the table keeps it open.
// -------------------------------------------------------------

#[derive(Deserialize)]
pub struct ReactForm {
    pub card_id: String,
    pub play: u32,
}

/// `POST /room/{code}/lastcall/react` — public (`persist_and_broadcast_lc`,
/// see the section comment above). `ctx.st.play_reaction` carries every
/// guard (seated/alive/beat/card/target/afford).
pub async fn lc_react_handler(
    State(state): State<GameState>,
    PlayerSession(player): PlayerSession,
    Path(code): Path<String>,
    Form(form): Form<ReactForm>,
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
    if let Err(e) = ctx.st.play_reaction(player.id, &form.card_id, form.play) {
        return map_lc(e);
    }
    persist_and_broadcast_lc(&state, &ctx).await;
    StatusCode::NO_CONTENT.into_response()
}

#[derive(Deserialize)]
pub struct HauntForm {
    pub play: u32,
}

/// `POST /room/{code}/lastcall/haunt` — public, same rationale as
/// `lc_react_handler` above: a ghost's vote is legible the instant it's
/// cast (I10).
pub async fn lc_haunt_handler(
    State(state): State<GameState>,
    PlayerSession(player): PlayerSession,
    Path(code): Path<String>,
    Form(form): Form<HauntForm>,
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
    if let Err(e) = ctx.st.haunt(player.id, form.play) {
        return map_lc(e);
    }
    persist_and_broadcast_lc(&state, &ctx).await;
    StatusCode::NO_CONTENT.into_response()
}

#[derive(Deserialize)]
pub struct ChallengeVoteForm {
    /// The challenge's identity token (`ChallengeState::key`) — echoed from
    /// the vote buttons so a stale screen's vote can't land on the next
    /// queued challenge (review wave; the `ReactForm.play` precedent).
    pub challenge: u64,
    pub for_instigator: bool,
}

/// `POST /room/{code}/lastcall/challenge-vote` (challenge-cards container)
/// — public (`persist_and_broadcast_lc`): a vote moves the tally every
/// surface renders, and the settling vote moves everything.
/// `ctx.st.challenge_vote` carries every guard (seated/alive/active
/// challenge/contestant/once).
pub async fn lc_chvote_handler(
    State(state): State<GameState>,
    PlayerSession(player): PlayerSession,
    Path(code): Path<String>,
    Form(form): Form<ChallengeVoteForm>,
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
    if let Err(e) = ctx
        .st
        .challenge_vote(player.id, form.challenge, form.for_instigator)
    {
        return map_lc(e);
    }
    persist_and_broadcast_lc(&state, &ctx).await;
    StatusCode::NO_CONTENT.into_response()
}

#[derive(Deserialize)]
pub struct GrantForm {
    pub card_id: String,
}

/// `POST /room/{code}/lastcall/test/grant` — test play mode only (404
/// otherwise, the `test_spawn`/`test_act_as` rule): push any catalog card
/// into the caller's own hand, so `copies: 0` challenge prototypes are
/// playable without waiting for Pack 3's shoe balance. Tick-only
/// broadcast — a hand is private (E5/E6).
pub async fn lc_test_grant_handler(
    State(state): State<GameState>,
    PlayerSession(player): PlayerSession,
    Path(code): Path<String>,
    Form(form): Form<GrantForm>,
) -> axum::response::Response {
    if !state.test_mode {
        return StatusCode::NOT_FOUND.into_response();
    }
    let lock = match lc_lock(&state, &code).await {
        Ok(l) => l,
        Err(r) => return r,
    };
    let _guard = lock.lock().await;
    let mut ctx = match load_lc(&state, &code, &player).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let Some(card) = crate::lc_cards::card_by_id(&form.card_id) else {
        return StatusCode::UNPROCESSABLE_ENTITY.into_response();
    };
    let Some(seat) = ctx.st.seat_of(player.id) else {
        return map_lc(LcError::NotSeated);
    };
    if ctx.st.players[seat].status != Status::Alive {
        return map_lc(LcError::NotAlive);
    }
    ctx.st.players[seat].hand.push(card);
    ctx.st.seq += 1;
    persist_and_tick_lc(&state, &ctx).await;
    StatusCode::NO_CONTENT.into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Review wave: SEND BACK's button gate mirrors `play_reaction`'s
    /// carve-out exactly — offered against a target-less DUEL challenge
    /// (the role swap), refused against a Solo (dead spend) and against a
    /// target-less numeric play (nothing to send home to).
    #[test]
    fn test_scope_legal_offers_reflect_against_duel_challenges_only() {
        let reflect = Some(crate::lc_cards::ReactionFx::Reflect);
        let wine08 = crate::lc_cards::card_by_id("wine-08").unwrap();
        let play = |id: &str| Play {
            card: crate::lc_cards::card_by_id(id).unwrap(),
            source_seat: 0,
            target: None,
            paid_from: Deck::Liquor,
            order_key: 1,
        };
        assert!(scope_legal(1, 3, &wine08, reflect, &play("liquor-09")));
        assert!(!scope_legal(1, 3, &wine08, reflect, &play("soft-09")));
        assert!(!scope_legal(1, 3, &wine08, reflect, &play("beer-05")));
    }

    /// 3 players with vessels, round bumped to 2. Walks the whole chain,
    /// asserting Deal never surfaces as a separate stop (E4: the chain
    /// collapses it in the same pass as the user-visible advance) and that
    /// NO stop arms a deadline — the clock is gone (2026-08-13), including
    /// for a blob that arrives holding a stale pre-removal deadline.
    #[test]
    fn test_advance_chain_walks_beats_untimed_and_skips_auto_ones() {
        let mut st = LastCallState::new(vec![(1, "a".into()), (2, "b".into()), (3, "c".into())], 1);
        st.set_vessel(1, Deck::Beer, "can").unwrap();
        st.set_vessel(2, Deck::Cider, "bottle").unwrap();
        st.set_vessel(3, Deck::Wine, "glass").unwrap();
        st.round = 2;
        st.beat_deadline_ms = Some(1_000_000); // stale pre-removal blob

        lc_advance_chain(&mut st);
        assert_eq!(st.beat, Beat::Diplomacy, "Deal must be skipped");
        assert_eq!(st.beat_deadline_ms, None, "the chain sweeps stale clocks");

        lc_advance_chain(&mut st);
        assert_eq!(st.beat, Beat::Reveal, "Diplomacy exits straight to Reveal");
        assert_eq!(st.beat_deadline_ms, None);

        // From Reveal: advance_beat (-> Resolve), then the loop's own
        // resolve() branch rolls the round over.
        lc_advance_chain(&mut st);
        assert_eq!(st.round, 3);
        assert_eq!(st.beat, Beat::Draw);
        assert_eq!(st.beat_deadline_ms, None, "round >= 2 Draw is untimed too");
    }

    /// From Reveal, a chain that ends the game (resolve() sets an outcome)
    /// must freeze the clock (`None`) rather than arm a deadline nobody will
    /// ever see counted down, and must leave `beat` at `Resolve` — the
    /// engine's own frozen-tableau shape (D16), untouched by the chain's
    /// loop exit. 2 players, bob's hp lowered to 1 so alice's locked
    /// beer-01 (2 damage, per `test_resolve_applies_damage_and_rolls_over`)
    /// finishes him off during `resolve()` — the same `arm`/`set_target`/
    /// `lock_in` staging `locked_table()` uses, not a hand-rolled effect.
    #[test]
    fn test_advance_chain_freezes_on_game_over() {
        let mut st = LastCallState::new(vec![(1, "alice".into()), (2, "bob".into())], 1);
        st.set_vessel(1, Deck::Beer, "can").unwrap();
        st.set_vessel(2, Deck::Cider, "bottle").unwrap();
        st.beat = Beat::Lock;
        st.arm(1, "beer-01").unwrap();
        st.set_target(1, "beer-01", Some(1)).unwrap();
        st.lock_in(1).unwrap();
        st.lock_in(2).unwrap(); // bob locks nothing armed — legal
        st.players[1].hp = 1; // one hit from dead

        lc_advance_chain(&mut st); // Lock -> Reveal
        assert_eq!(st.beat, Beat::Reveal);
        lc_advance_chain(&mut st); // Reveal -> advance(Resolve) -> resolve()

        assert_eq!(st.outcome(), Some(crate::last_call::LcOutcome::Winner(0)));
        assert_eq!(st.beat, Beat::Resolve);
        assert_eq!(st.beat_deadline_ms, None);
    }

    /// I1 (review): `resolve()`'s M3 hardening makes an empty-`players`
    /// state a permanent, silent no-op — `Ok(())`, `beat` untouched, `seq`
    /// not bumped — and `outcome()` is `None` below two players. Without
    /// `lc_advance_chain`'s own empty-players early return, a chain entered
    /// at `Beat::Resolve` with no players would loop forever with no
    /// `.await` inside it. Run on a background thread with a bounded
    /// `recv_timeout` rather than calling directly in-test: against the
    /// unfixed code this call never returns, and an in-test infinite loop
    /// would hang the whole suite instead of failing this one test.
    #[test]
    fn test_advance_chain_returns_immediately_on_empty_players() {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut st = LastCallState {
                players: vec![],
                beat: Beat::Resolve,
                ..Default::default()
            };
            lc_advance_chain(&mut st);
            let _ = tx.send(st);
        });
        let st = rx
            .recv_timeout(std::time::Duration::from_secs(2))
            .expect("lc_advance_chain spun instead of returning on empty players");
        assert!(st.players.is_empty());
        assert_eq!(st.beat, Beat::Resolve); // untouched — M3's no-op shape
        assert_eq!(st.beat_deadline_ms, None); // never armed
    }

    /// I2 (review): a deterministic two-actor contention test for the
    /// ticker/route race this task exists to get right. `lc_tick_room` is
    /// `pub(crate)` — visible here, not from the external `tests/http.rs`
    /// crate — so this lives in-crate rather than as an integration test.
    ///
    /// Determinism caveat, stated plainly so nobody chases a false
    /// guarantee: the ASSERTION is deterministic regardless of scheduling —
    /// the ticker cannot mutate `LastCallState` until this test body's guard
    /// drops, and once it does, `lc_tick_room`'s post-lock reload sees the
    /// already-advanced state and its recheck no-ops. The 50ms sleep below
    /// only raises the probability that the branch actually exercised is
    /// the LOCKED recheck (parked on `lock.lock().await`) rather than the
    /// lock-free advisory early-return finishing first — four orders of
    /// magnitude of headroom over one in-memory SQLite `SELECT`, so this
    /// will not flake, but the sleep is not what makes the test correct.
    #[tokio::test]
    async fn test_ticker_and_a_route_do_not_double_advance() {
        let pool = crate::db::test_pool().await;
        let alice = db::insert_player(&pool, "alice", "h").await.unwrap();
        let bob = db::insert_player(&pool, "bob", "h").await.unwrap();
        let room = crate::rooms::create_room_with_unique_code(&pool).await;
        db::join_room(&pool, room.id, alice).await;
        db::join_room(&pool, room.id, bob).await;

        let mut st = LastCallState::new(vec![(alice, "alice".into()), (bob, "bob".into())], 1);
        st.set_vessel(alice, Deck::Beer, "can").unwrap();
        st.set_vessel(bob, Deck::Cider, "bottle").unwrap();
        st.beat = Beat::Lock;
        st.beat_deadline_ms = Some(now_ms() - 2_000); // already expired
        let game_id = db::start_game(&pool, room.id, "last_call", "", "", Some(&st.to_json()))
            .await
            .unwrap();

        let state = crate::GameState {
            pool: pool.clone(),
            hub: crate::hub::RoomHub::new(),
            base_path: std::sync::Arc::from("/drinks"),
            locks: crate::RoomLocks::default(),
            test_mode: false,
        };

        // The test body takes the room guard FIRST — it plays the winning
        // route.
        let guard_lock = state.locks.for_room(room.id);
        let guard = guard_lock.lock().await;

        let ticker_state = state.clone();
        let room_id = room.id;
        let ticker = tokio::spawn(async move {
            lc_tick_room(&ticker_state, room_id).await;
        });

        // Let the ticker run its lock-free advisory read and park on the
        // guard this test body is holding.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Act as the winning route: reload, advance Lock -> Reveal, persist
        // — all still under the guard the ticker is waiting on.
        let game = db::get_active_game(&pool, room.id).await.unwrap();
        let mut route_st = LastCallState::from_json(game.state_json.as_deref().unwrap());
        lc_advance_chain(&mut route_st);
        db::set_game_state(&pool, game_id, &route_st.to_json()).await;
        drop(guard);

        ticker.await.unwrap();

        let after = LastCallState::from_json(
            db::get_active_game(&pool, room.id)
                .await
                .unwrap()
                .state_json
                .as_deref()
                .unwrap(),
        );
        // Exactly one advance — Lock -> Reveal, never Resolve and never a
        // round+1 Draw — and the stale deadline the route's chain swept to
        // `None` is what the ticker's recheck saw (`is_none_or` -> not due):
        // proof the ticker no-opped rather than double-advancing.
        assert_eq!(after.beat, Beat::Reveal);
        assert_eq!(after.beat_deadline_ms, None);
    }

    /// alice(1)/bob(2)/cara(3)/dave(4) -> seats 0-3, the same shape as
    /// `last_call.rs`'s own `at_diplomacy()` test fixture — private to that
    /// module, so redefined here rather than reached across the crate.
    fn diplomacy_state() -> LastCallState {
        let mut st = LastCallState::new(
            vec![
                (1, "alice".into()),
                (2, "bob".into()),
                (3, "cara".into()),
                (4, "dave".into()),
            ],
            42,
        );
        st.set_vessel(1, Deck::Beer, "can").unwrap();
        st.set_vessel(2, Deck::Cider, "bottle").unwrap();
        st.set_vessel(3, Deck::Soft, "glass").unwrap();
        st.set_vessel(4, Deck::Liquor, "shot").unwrap();
        st.beat = Beat::Diplomacy;
        st
    }

    /// Plan G whole-plan-review erratum (originally found by Task 3, fixed
    /// as a cross-task seam repair): `resolve()` used to stamp
    /// `PactBreak { round: self.round }` in Step 1, BEFORE Step 8's
    /// rollover bumps `self.round` in the same call — and since
    /// `lc_advance_chain` never persists an intermediate state between the
    /// two (Reveal -> Resolve -> the round+1 Draw is one synchronous pass),
    /// no client fetch could ever observe `st.round` still equal to the
    /// round a non-terminal betrayal was recorded against. The round-scoped
    /// reader (`lc_screen_panel`'s break strip, filtering
    /// `round == view.round` verbatim per the brief; the retired pact
    /// section's betrayed notice was its twin) was unreachable for every betrayal except
    /// one that also happened to end the game — contradicting G5 ("loud, by
    /// name"). `resolve()` now re-stamps a non-terminal break with the round
    /// it rolls over INTO (the round players actually land on) rather than
    /// the round it was thrown in; a terminal break keeps the round the game
    /// froze on. This test pins the fixed contract: a non-terminal betrayal
    /// is loud for exactly the one round following it, then ages out —
    /// loud, not permanent (G5).
    #[test]
    fn test_a_non_terminal_betrayal_is_loud_for_the_following_round() {
        let mut st = diplomacy_state();
        st.offer_pact(1, 1).unwrap(); // alice (seat 0) -> bob (seat 1)
        st.accept_pact(2, 0).unwrap();
        st.beat = Beat::Lock;
        st.arm(1, "beer-01").unwrap(); // Damage, targets "one"
        st.set_target(1, "beer-01", Some(1)).unwrap(); // alice aims at bob
        st.lock_in(1).unwrap();
        st.advance_beat().unwrap(); // Reveal
        st.advance_beat().unwrap(); // Resolve
        st.resolve().unwrap(); // non-terminal: bob survives, round rolls over

        let brk = *st.pact_breaks.last().unwrap();
        assert_eq!(
            brk.round, st.round,
            "a non-terminal break must be stamped with the round it rolls \
             over into, not the round it was thrown in"
        );

        // The round-scoped surface now shows the betrayal in the very
        // first frame anyone can fetch it in. (The private pact section was
        // the other reader until the 2026-08-13 screen-declutter pack
        // retired the pact UI; the break strip is the one left.)
        let view = st.public_view();
        assert!(lc_render::lc_screen_panel(&view).contains("lc-pact-break"));

        // One more resolve (no new betrayal) rolls the round over again —
        // the break ages out of both surfaces, per G5 ("loud", not
        // "permanent").
        st.beat = Beat::Resolve;
        st.resolve().unwrap();
        assert_ne!(st.pact_breaks.last().unwrap().round, st.round);
        assert!(!lc_render::lc_screen_panel(&st.public_view()).contains("lc-pact-break"));
    }
}
