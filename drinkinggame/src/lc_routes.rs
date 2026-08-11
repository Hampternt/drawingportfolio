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
use crate::last_call::{Deck, LastCallState, LcError, PublicView};
use crate::lc_render::{self, HandGroupView, SetupRow};
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
            _ => StatusCode::UNPROCESSABLE_ENTITY.into_response(),
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

/// The rows come from the state itself, not a second `room_members` query —
/// `LastCallState.players` already carries name, handicap and vessels, and
/// using it keeps the shell and the hand fragment reading one source.
fn setup_rows(st: &LastCallState) -> Vec<SetupRow> {
    st.players
        .iter()
        .map(|p| SetupRow {
            player_id: p.player_id,
            name: p.name.clone(),
            handicap_pct: p.handicap_pct,
            decks: p.vessels.iter().map(|v| v.deck).collect(),
        })
        .collect()
}

/// The single builder of the `#lc-hand` fragment — mirrors
/// `table_pane_html`'s role, so the shell's initial paint (`lc_page`) and the
/// per-viewer refetch (`lc_hand_handler`) can never disagree about the
/// fragment's shape for the same state. Closes the STATUS-carried
/// "rows-and-hand lookup duplicated verbatim" minor from Plan A2.
fn hand_pane_html(base_path: &str, code: &str, st: &LastCallState, player_id: i64) -> String {
    let rows = setup_rows(st);
    let (hand, armed, locked, handicap_pct) = match st.seat_of(player_id) {
        Some(seat) => {
            let p = &st.players[seat];
            (
                p.hand.as_slice(),
                p.armed.iter().map(|a| a.card.clone()).collect::<Vec<_>>(),
                p.locked,
                p.handicap_pct,
            )
        }
        None => (&[] as &[_], Vec::new(), false, 100),
    };
    let hg = HandGroupView {
        hand,
        armed: &armed,
        locked,
        handicap_pct,
    };
    lc_render::lc_hand_pane(base_path, code, player_id, &hg, &rows, st.seq)
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
    banner: String,     // lc_render::lc_banner(&view)
    hand_pane: String,  // lc_render::lc_hand_pane(...)
    table_pane: String, // table_pane_html(&view, me) — the #lc-table fragment
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
    let me = ctx.st.seat_of(player.id);
    let hand_pane = hand_pane_html(&state.base_path, &code, &ctx.st, player.id);
    let view = ctx.st.public_view();
    let tpl = LcRoomTemplate {
        base_path: state.base_path.to_string(),
        code,
        player_id: player.id,
        banner: lc_render::lc_banner(&view),
        hand_pane,
        table_pane: table_pane_html(&view, me),
    };
    Html(tpl.render().unwrap()).into_response()
}

/// The `#lc-table` fragment: the F.3 mini table (`lc_render::lc_mini_table`)
/// wrapped with the `data-seq` freshness marker, mirroring `lc_hand_pane`'s
/// `#lc-hand` root. Shared by `lc_page` (initial paint) and
/// `lc_table_handler` (the per-viewer refetch) so the two can never
/// disagree on the fragment's shape for the same state.
fn table_pane_html(view: &PublicView, me: Option<usize>) -> String {
    format!(
        r#"<div id="lc-table" data-seq="{}">{}</div>"#,
        view.seq,
        lc_render::lc_mini_table(view, me),
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
    let me = ctx.st.seat_of(player.id);
    let view = ctx.st.public_view();
    Html(table_pane_html(&view, me)).into_response()
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
