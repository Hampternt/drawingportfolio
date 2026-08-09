//! Last Call route handlers. `/lastcall/start`, `/lastcall/vessel` and
//! `/lastcall/handicap` land here (Task 1); the shell page, hand fragment
//! and the beat-loop action routes are later tasks. SQL stays in db.rs; HTML
//! fragments stay in lc_render.rs.

use axum::extract::{Form, Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect};
use rand::Rng;
use serde::Deserialize;

use crate::auth::PlayerSession;
use crate::db;
use crate::error::GameError;
use crate::last_call::{Deck, LastCallState, LcError};
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
/// the ROOM/TABLE panel — mode = "last_call" for as long as this game's
/// phone panel is still the Task 1 placeholder (Step 1). Task 3 adds the
/// LcPublic / LcTick publishes here; the signature does not change.
pub(crate) async fn persist_and_broadcast_lc(state: &GameState, ctx: &LcCtx) {
    db::set_game_state(&state.pool, ctx.game.id, &ctx.st.to_json()).await;
    db::touch_room(&state.pool, ctx.room.id).await;
    crate::game::broadcast_room(state, ctx.room.id, &ctx.room.code).await;
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
    // rand::thread_rng() here is the *only* randomness this feature has —
    // last_call.rs never generates its own; rng_seed is taken once, in the
    // route, and stored in the state blob.
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

#[derive(Deserialize)]
pub struct VesselForm {
    pub deck: String,
    pub container: String,
}

/// Registers what a seated player is drinking. Follows `tm_mode_handler`'s
/// gate-then-validate shape: `load_lc` (member/active-game/kind gating) runs
/// BEFORE the form's own field validation, so a non-member or a wrong-kind
/// game gets its 403/409 rather than a 422 that would leak "this form field
/// is invalid" to a request with no business hitting this room at all.
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
        return match e {
            LcError::NotSeated => GameError::NotYourCall.into_response(),
            LcError::BadHandicap | LcError::NotImplemented => {
                StatusCode::UNPROCESSABLE_ENTITY.into_response()
            }
        };
    }
    persist_and_broadcast_lc(&state, &ctx).await;
    Redirect::to(&format!("{}/room/{}/lastcall", state.base_path, code)).into_response()
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

    if ctx.st.set_handicap(form.target, form.handicap_pct).is_err() {
        return StatusCode::UNPROCESSABLE_ENTITY.into_response();
    }
    persist_and_broadcast_lc(&state, &ctx).await;
    Redirect::to(&format!("{}/room/{}/lastcall", state.base_path, code)).into_response()
}
