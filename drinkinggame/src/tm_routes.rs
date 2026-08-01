//! 3 Man route handlers. `/tm/start` and `/tm/end` land here (Task 12); the
//! in-game action routes (`/tm/roll`, `/tm/pass`, `/tm/three-man`,
//! `/tm/mode`, `/tm/target`, `/tm/clear-slot`, `/tm/send`, `/tm/gift-roll`,
//! `/tm/seat`) are Task 13. SQL stays in db.rs; HTML fragments stay in
//! render.rs.

use axum::extract::{Form, Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use rand::Rng;
use serde::Deserialize;

use crate::auth::PlayerSession;
use crate::db;
use crate::error::GameError;
use crate::hub::RoomMessage;
use crate::models::{Game, Player, Room};
use crate::render;
use crate::three_man::{GiveMode, Phase, ThreeManState, TmError};
use crate::GameState;

/// Everything an action handler needs to read and mutate a running 3 Man
/// game: the room (for id/code), the raw `games` row (for its id), and the
/// parsed state (mutated in place by the handler, then persisted).
pub(crate) struct TmCtx {
    pub room: Room,
    pub game: Game,
    pub st: ThreeManState,
}

/// member_room -> active game -> kind == "three_man" else WrongGameKind ->
/// parse state. Shared entry point for every `/tm/*` action handler.
pub(crate) async fn load_tm(
    state: &GameState,
    code: &str,
    player: &Player,
) -> Result<TmCtx, axum::response::Response> {
    let room = crate::game::member_room(state, code, player).await?;
    let Some(game) = db::get_active_game(&state.pool, room.id).await else {
        return Err(GameError::NoActiveGame.into_response());
    };
    if game.kind != "three_man" {
        return Err(GameError::WrongGameKind.into_response());
    }
    let st = ThreeManState::from_json(game.state_json.as_deref().unwrap_or_default());
    Ok(TmCtx { room, game, st })
}

/// Persists the mutated state back to the DB, then re-renders and publishes
/// every surface that reflects it: phone GAME tab, big screen, ROOM/TABLE
/// tab (topbar mode flip + seating), and the leaderboard's 3 MAN badge.
/// Shared by every action handler that mutates `ThreeManState` in place —
/// Task 13's roll/pass/hand-off/assign/gift-roll/seat routes, and
/// `tm_start_handler` below.
pub(crate) async fn persist_and_broadcast(state: &GameState, ctx: &TmCtx) {
    db::set_game_state(&state.pool, ctx.game.id, &ctx.st.to_json()).await;
    db::touch_room(&state.pool, ctx.room.id).await;
    crate::game::broadcast_game(state, ctx.room.id, &ctx.room.code, None).await;
    crate::game::broadcast_room(state, ctx.room.id, &ctx.room.code).await;
    crate::routes::broadcast_leaderboard(state, ctx.room.id).await;
}

/// member_room -> room_members (>= 2 else TooFewPlayers) ->
/// ThreeManState::new(member_ids_by_joined_at, player.id) -> start_game ->
/// touch, broadcast all. Locked across the whole body: a concurrent join
/// mutating the room's member list between the count check and the seed
/// must not race a start.
pub async fn tm_start_handler(
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
    let member_ids: Vec<i64> = members.iter().map(|m| m.id).collect();
    let st = ThreeManState::new(member_ids, player.id);
    // deck_order/rules_json are Ring of Fire concepts — 3 Man leaves both
    // empty; GameAlreadyActive races are handled by the games table's
    // partial unique index (one active game per room).
    if let Err(e) = db::start_game(
        &state.pool,
        room.id,
        "three_man",
        "",
        "",
        Some(&st.to_json()),
    )
    .await
    {
        return e.into_response();
    }

    // Re-load under the lock rather than hand-assembling a Game/TmCtx: this
    // doubles as proof persist_and_broadcast works for a freshly-started
    // game, the same helper every future action handler reuses.
    let ctx = match load_tm(&state, &code, &player).await {
        Ok(c) => c,
        Err(resp) => return resp,
    };
    persist_and_broadcast(&state, &ctx).await;
    StatusCode::NO_CONTENT.into_response()
}

/// Assembles 3 Man post-game superlatives purely from the room leaderboard
/// — unlike Ring of Fire's `game_summary`, there's no draws/kings table to
/// join against (3 Man games never write to `game_draws`).
async fn tm_summary(state: &GameState, room_id: i64) -> render::TmOverView {
    let rows = db::leaderboard(&state.pool, room_id).await;
    let hardest = rows.first().map(|r| (r.name.clone(), r.drinks, r.shots));
    let most_shots = rows
        .iter()
        .max_by_key(|r| r.shots)
        .map(|r| (r.name.clone(), r.shots));
    let room_total: i64 = rows.iter().map(|r| r.drinks + r.shots).sum();
    render::TmOverView {
        hardest,
        most_shots,
        room_total,
    }
}

/// load_tm -> end_game -> build TmOverView from the leaderboard -> broadcast
/// the over panel + idle panel on `game`, the over panel on `screen`, then a
/// room refresh. Locked across the whole body: it can otherwise race a
/// gift_roll (Task 13) persisting state onto a game this request just
/// ended.
pub async fn tm_end_handler(
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

    let ctx = match load_tm(&state, &code, &player).await {
        Ok(c) => c,
        Err(resp) => return resp,
    };
    db::end_game(&state.pool, ctx.game.id).await;
    db::touch_room(&state.pool, room.id).await;

    let summary = tm_summary(&state, room.id).await;
    let phone_html = format!(
        "{}{}",
        render::tm_over_panel(&summary),
        crate::game::idle_panel(&state, &room.code).await
    );
    let screen_html = render::tm_screen_over(&summary);
    state.hub.publish(room.id, RoomMessage::Game(phone_html));
    state.hub.publish(room.id, RoomMessage::Screen(screen_html));
    crate::game::broadcast_room(&state, room.id, &room.code).await;
    // The game is now ended, so this re-render is kind-aware via
    // get_active_game returning None — it drops the outgoing 3 Man's
    // standings badge instead of leaving it stuck until the next
    // unrelated drink/undo broadcast.
    crate::routes::broadcast_leaderboard(&state, room.id).await;
    StatusCode::NO_CONTENT.into_response()
}

// -------------------------------------------------------------
// Task 13: in-game action routes. Every handler below follows the same
// shape — resolve the room, take its lock, re-load state under the lock,
// gate the actor, attempt the engine transition, auto-log any drinks the
// transition produced, then persist_and_broadcast once. All state mutation
// through broadcast happens while the guard is held.
// -------------------------------------------------------------

/// `WrongPhase`/`BadTarget` both mean "that move isn't legal right now" —
/// OutOfTurn (409) covers both; TooFewPlayers keeps its own 409 variant.
fn map_tm(e: TmError) -> GameError {
    match e {
        TmError::WrongPhase | TmError::BadTarget => GameError::OutOfTurn,
        TmError::TooFewPlayers => GameError::TooFewPlayers,
    }
}

/// Resolves the room and hands back its per-room lock — acquired by the
/// caller so everything from re-load through persist runs under one guard.
async fn tm_lock(
    state: &GameState,
    code: &str,
) -> Result<std::sync::Arc<tokio::sync::Mutex<()>>, axum::response::Response> {
    let Some(room) = db::get_open_room(&state.pool, &code.to_uppercase()).await else {
        return Err(GameError::RoomNotFound.into_response());
    };
    Ok(state.locks.for_room(room.id))
}

/// Actor gate shared by mode/target/clear-slot/send: only the double's
/// owner may act on it. Gates on `phase == Assign` first, not just
/// `double.is_some()` — `double` is only cleared at the *start* of the next
/// `roll()`, not by `pass()`, so a finished double sits there as
/// `Some(stale_owner)` for the whole window between the gift round ending
/// and the next roll (phase back to `Ready`); without the phase check a
/// stranger to that dead double would wrongly get 403 NotYourCall instead
/// of 409 OutOfTurn ("no double running"). Returns `GameError` rather than
/// a pre-rendered `Response` — a bare enum keeps this `Result` small
/// (clippy::result_large_err), unlike the ~128-byte axum `Response`.
fn require_double_owner(ctx: &TmCtx, player_id: i64) -> Result<(), GameError> {
    if ctx.st.phase != Phase::Assign {
        return Err(GameError::OutOfTurn);
    }
    match ctx.st.double.as_ref() {
        Some(d) if d.owner == player_id => Ok(()),
        Some(_) => Err(GameError::NotYourCall),
        None => Err(GameError::OutOfTurn),
    }
}

/// Any room member may tap ROLL for the table — it doesn't have to be
/// whoever's turn it nominally is. Auto-logs every `Call` the roll produced
/// (3s, 7/9/11) before persisting.
pub async fn tm_roll_handler(
    State(state): State<GameState>,
    PlayerSession(player): PlayerSession,
    Path(code): Path<String>,
) -> axum::response::Response {
    let lock = match tm_lock(&state, &code).await {
        Ok(l) => l,
        Err(r) => return r,
    };
    let _guard = lock.lock().await;
    let mut ctx = match load_tm(&state, &code, &player).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let (d1, d2) = {
        let mut rng = rand::thread_rng();
        (rng.gen_range(1..=6), rng.gen_range(1..=6))
    };
    if let Err(e) = ctx.st.roll(d1, d2) {
        return map_tm(e).into_response();
    }
    for call in &ctx.st.calls {
        db::insert_events_bulk(
            &state.pool,
            ctx.room.id,
            call.player_id,
            "drink",
            call.amount as u32,
        )
        .await;
    }
    persist_and_broadcast(&state, &ctx).await;
    StatusCode::NO_CONTENT.into_response()
}

#[derive(Deserialize)]
pub struct TargetOnlyForm {
    pub target: i64,
}

/// HandOff (a lone 3 landed on the current 3 Man): only the roller may hand
/// the title off, via `give_three_man`. Any other time this is a free
/// table-tab reassign any member can make, via `set_three_man` — which also
/// tolerates being called mid-HandOff (an engine-level convenience), but the
/// route's own gate above it is what actually restricts who may use that
/// path while HandOff is active.
pub async fn tm_three_man_handler(
    State(state): State<GameState>,
    PlayerSession(player): PlayerSession,
    Path(code): Path<String>,
    Form(form): Form<TargetOnlyForm>,
) -> axum::response::Response {
    let lock = match tm_lock(&state, &code).await {
        Ok(l) => l,
        Err(r) => return r,
    };
    let _guard = lock.lock().await;
    let mut ctx = match load_tm(&state, &code, &player).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let result = if ctx.st.phase == Phase::HandOff {
        if player.id != ctx.st.roller() {
            return GameError::NotYourCall.into_response();
        }
        ctx.st.give_three_man(form.target)
    } else {
        ctx.st.set_three_man(form.target)
    };
    if let Err(e) = result {
        return map_tm(e).into_response();
    }
    persist_and_broadcast(&state, &ctx).await;
    StatusCode::NO_CONTENT.into_response()
}

#[derive(Deserialize)]
pub struct ModeForm {
    pub mode: String,
}

pub async fn tm_mode_handler(
    State(state): State<GameState>,
    PlayerSession(player): PlayerSession,
    Path(code): Path<String>,
    Form(form): Form<ModeForm>,
) -> axum::response::Response {
    let lock = match tm_lock(&state, &code).await {
        Ok(l) => l,
        Err(r) => return r,
    };
    let _guard = lock.lock().await;
    let mut ctx = match load_tm(&state, &code, &player).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    if let Err(e) = require_double_owner(&ctx, player.id) {
        return e.into_response();
    }
    let mode = match form.mode.as_str() {
        "both" => GiveMode::Both,
        "split" => GiveMode::Split,
        _ => return StatusCode::UNPROCESSABLE_ENTITY.into_response(),
    };
    if let Err(e) = ctx.st.set_mode(mode) {
        return map_tm(e).into_response();
    }
    persist_and_broadcast(&state, &ctx).await;
    StatusCode::NO_CONTENT.into_response()
}

#[derive(Deserialize)]
pub struct TargetForm {
    pub slot: usize,
    pub target: i64,
}

pub async fn tm_target_handler(
    State(state): State<GameState>,
    PlayerSession(player): PlayerSession,
    Path(code): Path<String>,
    Form(form): Form<TargetForm>,
) -> axum::response::Response {
    let lock = match tm_lock(&state, &code).await {
        Ok(l) => l,
        Err(r) => return r,
    };
    let _guard = lock.lock().await;
    let mut ctx = match load_tm(&state, &code, &player).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    if let Err(e) = require_double_owner(&ctx, player.id) {
        return e.into_response();
    }
    if let Err(e) = ctx.st.pick_target(form.slot, form.target) {
        return map_tm(e).into_response();
    }
    persist_and_broadcast(&state, &ctx).await;
    StatusCode::NO_CONTENT.into_response()
}

#[derive(Deserialize)]
pub struct SlotForm {
    pub slot: usize,
}

pub async fn tm_clear_slot_handler(
    State(state): State<GameState>,
    PlayerSession(player): PlayerSession,
    Path(code): Path<String>,
    Form(form): Form<SlotForm>,
) -> axum::response::Response {
    let lock = match tm_lock(&state, &code).await {
        Ok(l) => l,
        Err(r) => return r,
    };
    let _guard = lock.lock().await;
    let mut ctx = match load_tm(&state, &code, &player).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    if let Err(e) = require_double_owner(&ctx, player.id) {
        return e.into_response();
    }
    if let Err(e) = ctx.st.clear_slot(form.slot) {
        return map_tm(e).into_response();
    }
    persist_and_broadcast(&state, &ctx).await;
    StatusCode::NO_CONTENT.into_response()
}

pub async fn tm_send_handler(
    State(state): State<GameState>,
    PlayerSession(player): PlayerSession,
    Path(code): Path<String>,
) -> axum::response::Response {
    let lock = match tm_lock(&state, &code).await {
        Ok(l) => l,
        Err(r) => return r,
    };
    let _guard = lock.lock().await;
    let mut ctx = match load_tm(&state, &code, &player).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    if let Err(e) = require_double_owner(&ctx, player.id) {
        return e.into_response();
    }
    if let Err(e) = ctx.st.send() {
        return map_tm(e).into_response();
    }
    persist_and_broadcast(&state, &ctx).await;
    StatusCode::NO_CONTENT.into_response()
}

/// Any member may roll a gift's dice (not just the owner or the victim).
/// Auto-logs the victim's total; if this roll was the one that completed
/// every gift and it matched the double's value, also logs the owner's
/// payback.
pub async fn tm_gift_roll_handler(
    State(state): State<GameState>,
    PlayerSession(player): PlayerSession,
    Path(code): Path<String>,
    Form(form): Form<SlotForm>,
) -> axum::response::Response {
    let lock = match tm_lock(&state, &code).await {
        Ok(l) => l,
        Err(r) => return r,
    };
    let _guard = lock.lock().await;
    let mut ctx = match load_tm(&state, &code, &player).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let Some(double) = ctx.st.double.as_ref() else {
        return GameError::OutOfTurn.into_response();
    };
    let Some(gift) = double.gifts.get(form.slot) else {
        return GameError::OutOfTurn.into_response();
    };
    let dice_count = gift.dice_count;
    let victim = gift.player_id;
    let owner = double.owner;
    let values: Vec<u8> = {
        let mut rng = rand::thread_rng();
        (0..dice_count).map(|_| rng.gen_range(1..=6)).collect()
    };
    let total = match ctx.st.gift_roll(form.slot, values) {
        Ok(t) => t,
        Err(e) => return map_tm(e).into_response(),
    };
    db::insert_events_bulk(&state.pool, ctx.room.id, victim, "drink", total as u32).await;
    if let Some(payback) = ctx.st.double.as_ref().and_then(|d| d.payback) {
        db::insert_events_bulk(&state.pool, ctx.room.id, owner, "drink", payback as u32).await;
    }
    persist_and_broadcast(&state, &ctx).await;
    StatusCode::NO_CONTENT.into_response()
}

pub async fn tm_pass_handler(
    State(state): State<GameState>,
    PlayerSession(player): PlayerSession,
    Path(code): Path<String>,
) -> axum::response::Response {
    let lock = match tm_lock(&state, &code).await {
        Ok(l) => l,
        Err(r) => return r,
    };
    let _guard = lock.lock().await;
    let mut ctx = match load_tm(&state, &code, &player).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    if let Err(e) = ctx.st.pass() {
        return map_tm(e).into_response();
    }
    persist_and_broadcast(&state, &ctx).await;
    StatusCode::NO_CONTENT.into_response()
}

#[derive(Deserialize)]
pub struct SeatForm {
    pub target: i64,
    pub dir: i64,
}

pub async fn tm_seat_handler(
    State(state): State<GameState>,
    PlayerSession(player): PlayerSession,
    Path(code): Path<String>,
    Form(form): Form<SeatForm>,
) -> axum::response::Response {
    let lock = match tm_lock(&state, &code).await {
        Ok(l) => l,
        Err(r) => return r,
    };
    let _guard = lock.lock().await;
    let mut ctx = match load_tm(&state, &code, &player).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    if let Err(e) = ctx.st.move_seat(form.target, form.dir) {
        return map_tm(e).into_response();
    }
    persist_and_broadcast(&state, &ctx).await;
    StatusCode::NO_CONTENT.into_response()
}
