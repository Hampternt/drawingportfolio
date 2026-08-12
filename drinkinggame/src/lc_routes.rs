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
    pull_cost, Beat, Card, Deck, LastCallState, LcError, PublicView, Status, DRAW_PER_VESSEL,
    PACT_MIN_ALIVE,
};
use crate::lc_render::{self, ActionBarView, HandGroupView, SetupRow};
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
fn hand_pane_html(base_path: &str, code: &str, st: &LastCallState, player_id: i64) -> String {
    let rows = setup_rows(st);
    let seat = st.seat_of(player_id);
    let (hand, armed, locked, handicap_pct) = match seat {
        Some(seat) => {
            let p = &st.players[seat];
            let armed_cards = if p.locked {
                st.staged_for(seat).into_iter().cloned().collect()
            } else {
                p.armed.iter().map(|a| a.card.clone()).collect::<Vec<_>>()
            };
            (p.hand.as_slice(), armed_cards, p.locked, p.handicap_pct)
        }
        None => (&[] as &[_], Vec::new(), false, 100),
    };
    let hg = HandGroupView {
        hand,
        armed: &armed,
        locked,
        handicap_pct,
    };
    let pane = lc_render::lc_hand_pane(base_path, code, player_id, &hg, &rows, st.seq);
    let targets = seat
        .map(|s| targets_section_html(st, s))
        .unwrap_or_default();
    let pacts = pacts_section_html(st, player_id);
    let bar = lc_render::lc_action_bar(&action_bar_view(st, player_id));
    format!(r#"{pane}{targets}{pacts}<template data-lc-actions>{bar}</template>"#)
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
            let charged: u8 = st
                .plays
                .iter()
                .filter(|play| play.source_seat == seat)
                .map(|play| pull_cost(play.card.cost, p.handicap_pct))
                .sum();
            ActionBarView {
                beat: st.beat,
                round: st.round,
                seated: true,
                alive: p.status == Status::Alive,
                locked: p.locked,
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
            }
        }
        None => ActionBarView {
            beat: st.beat,
            round: st.round,
            seated: false,
            alive: false,
            locked: false,
            drawing: false,
            vessels: Vec::new(),
            charged: 0,
            vessels_registered,
            outcome,
        },
    }
}

/// Plan E Task 4 / decision E8: the per-card seat `<select>` target picker,
/// appended to the hand pane. Empty string unless the viewer is mid-Lock,
/// unlocked, and has at least one armed `targets == "one"` card — outside
/// that window there is nothing to pick, and a locked player's picks are
/// already committed. Options are titled/named through `html_escape`, as
/// `lc_hand_pane` does for the same reason.
fn targets_section_html(st: &LastCallState, seat: usize) -> String {
    if st.beat != Beat::Lock {
        return String::new();
    }
    let p = &st.players[seat];
    if p.locked {
        return String::new();
    }
    let armed_one: Vec<_> = p.armed.iter().filter(|a| a.card.targets == "one").collect();
    if armed_one.is_empty() {
        return String::new();
    }
    let rows: String = armed_one
        .iter()
        .map(|a| {
            let options: String = st
                .players
                .iter()
                .filter(|tp| tp.status == Status::Alive)
                .map(|tp| {
                    let selected = if a.target == Some(tp.seat) {
                        " selected"
                    } else {
                        ""
                    };
                    format!(
                        r#"<option value="{seat}"{selected}>{name}</option>"#,
                        seat = tp.seat,
                        name = crate::render::html_escape(&tp.name),
                    )
                })
                .collect();
            format!(
                r#"<label class="lc-target-row"><span>{title}</span><select data-lc-target data-card-id="{id}"><option value="">PICK A TARGET</option>{options}</select></label>"#,
                title = crate::render::html_escape(&a.card.title),
                id = crate::render::html_escape(&a.card.id),
            )
        })
        .collect();
    format!(r#"<section class="lc-targets"><h2>Targets</h2>{rows}</section>"#)
}

/// A seat's name, uppercased and escaped — the `pacts_section_html` analogue
/// of `lc_render::seat_name_upper`, which reads `&PublicView` and so cannot
/// be reused here: every seat this function names comes from `pacts`/
/// `pact_offers`/`pact_barred`, none of which `PublicView` ever carries
/// (G13). `.get()`, not `[]`, for the same defensive reason `seat_name_upper`
/// gives — a stored seat index outliving the player it named is a corrupt-
/// blob concern, not a panic.
fn seat_name(st: &LastCallState, seat: usize) -> String {
    st.players
        .get(seat)
        .map(|p| crate::render::html_escape(&p.name.to_uppercase()))
        .unwrap_or_default()
}

/// Plan G, Task 3: the private pact section — the §7.8 "Pact" component,
/// Follows `targets_section_html`'s shape and placement (appended to the
/// hand pane, outside `#lc-hand`). Reads `pacts`/`pact_offers`/
/// `pact_barred` directly off `&LastCallState` — fields `PublicView` never
/// projects (G13) — so this builder, like `targets_section_html`, must never
/// be handed to anything but the viewer's own private hand fragment.
///
/// `""` for a non-member, and `""` again for a seated member with nothing to
/// show — the section is additive: each of the three parts below either
/// contributes a fragment of markup or contributes nothing, and an entirely
/// empty body means no `<section>` wrapper either.
fn pacts_section_html(st: &LastCallState, player_id: i64) -> String {
    let Some(seat) = st.seat_of(player_id) else {
        return String::new();
    };
    let mut body = String::new();

    // 1. Standing pact — any beat, while pacted. Read `pacts` directly
    // (rather than `pact_partner`, which drops `formed_round`) since the
    // line needs both the partner's seat and the round the pact formed.
    if let Some(pact) = st.pacts.iter().find(|p| p.a == seat || p.b == seat) {
        let partner = if pact.a == seat { pact.b } else { pact.a };
        body.push_str(&format!(
            r#"<p class="lc-pact-standing">PACT WITH {name} — SINCE ROUND {round}</p>"#,
            name = seat_name(st, partner),
            round = pact.formed_round,
        ));
    }

    // 2. Betrayed notice — any beat, current round only. A break record
    // outlives its round (Plan J's recap reads history), so this is
    // deliberately gated on `round == st.round`, not "any break naming me".
    if let Some(brk) = st
        .pact_breaks
        .iter()
        .find(|b| b.betrayed == seat && b.round == st.round)
    {
        body.push_str(&format!(
            r#"<p class="lc-pact-broken">{name} BROKE YOUR PACT</p>"#,
            name = seat_name(st, brk.betrayer),
        ));
    }

    // 3. The Diplomacy-only market — an Alive viewer only. Dead/eliminated
    // seats get nothing here (there is nothing left for them to negotiate),
    // and every beat but Diplomacy hides the market even for a live one.
    if st.beat == Beat::Diplomacy && st.players[seat].status == Status::Alive {
        if st.pact_barred.contains(&seat) {
            body.push_str(
                r#"<p class="lc-pact-barred">YOU BROKE A PACT — NOBODY DEALS WITH YOU NOW</p>"#,
            );
        } else if st.pact_partner(seat).is_none() {
            let alive_count = st
                .players
                .iter()
                .filter(|p| p.status == Status::Alive)
                .count();
            if alive_count >= PACT_MIN_ALIVE {
                for offer in st.pact_offers.iter().filter(|o| o.to == seat) {
                    body.push_str(&format!(
                        r#"<div class="lc-pact-offer-row"><span>{name} OFFERS A PACT</span><button class="lc-btn lc-pact-accept" data-lc-post="pact/accept" data-lc-body="from={from}">ACCEPT</button><button class="lc-btn lc-pact-decline" data-lc-post="pact/decline" data-lc-body="from={from}">DECLINE</button></div>"#,
                        name = seat_name(st, offer.from),
                        from = offer.from,
                    ));
                }
                let outgoing = st.pact_offers.iter().find(|o| o.from == seat);
                if let Some(offer) = outgoing {
                    body.push_str(&format!(
                        r#"<p class="lc-pact-pending">OFFERED TO {name} — WAITING</p>"#,
                        name = seat_name(st, offer.to),
                    ));
                }
                // Secretly-pacted seats stay listed (G11 — the list must not
                // be a pact detector); only publicly-barred seats and the
                // pending outgoing target (already rendered above as the
                // WAITING line) are dropped.
                for tp in st.players.iter().filter(|tp| tp.status == Status::Alive) {
                    if tp.seat == seat
                        || st.pact_barred.contains(&tp.seat)
                        || outgoing.is_some_and(|o| o.to == tp.seat)
                    {
                        continue;
                    }
                    body.push_str(&format!(
                        r#"<button class="lc-btn lc-pact-propose" data-lc-post="pact/offer" data-lc-body="target={target}">PROPOSE TO {name}</button>"#,
                        target = tp.seat,
                        name = seat_name(st, tp.seat),
                    ));
                }
            }
        }
        // else: pacted viewer — nothing beyond the standing line, no market.
    }

    if body.is_empty() {
        String::new()
    } else {
        format!(r#"<section class="lc-pacts"><h2>Pact</h2>{body}</section>"#)
    }
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
    actions: String,    // lc_render::lc_action_bar(&action_bar_view(&ctx.st, player.id))
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
    let actions = lc_render::lc_action_bar(&action_bar_view(&ctx.st, player.id));
    let view = ctx.st.public_view();
    let tpl = LcRoomTemplate {
        base_path: state.base_path.to_string(),
        code,
        player_id: player.id,
        banner: lc_render::lc_banner(&view),
        hand_pane,
        table_pane: table_pane_html(&view, me),
        actions,
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

/// Engine error -> HTTP. NotSeated/NotAlive are "you have no say here" (403,
/// like tm's NotYourCall); WrongBeat/AlreadyLocked/MustResolve are "not now"
/// (409, like tm's OutOfTurn); the two named-card refusals carry their
/// message as a plain-text 422 body the action bar shows verbatim (DDv2 6.3
/// "naming the card"); everything else (UnknownCard, NotPlayable, BadTarget,
/// BadDraw) is a bare 422. `lock_in` replay after a beat tick has already
/// moved past `Beat::Lock` returns `WrongBeat`, not the idempotent `Ok(())`
/// lock_in gives a same-beat replay — that's still "not now" from the
/// caller's side, so it takes the same 409 as every other WrongBeat case
/// rather than a special-cased mapping.
pub(crate) fn map_lc(e: LcError) -> axum::response::Response {
    match e {
        LcError::NotSeated | LcError::NotAlive => GameError::NotYourCall.into_response(),
        LcError::WrongBeat | LcError::AlreadyLocked | LcError::MustResolve => {
            GameError::OutOfTurn.into_response()
        }
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
    // Decision E3: the one engine-visible early beat exit. Every alive seat
    // locking before the 45s Lock deadline expires should not force the
    // table to sit out the rest of the timer with nothing left to decide.
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

// -------------------------------------------------------------
// Plan G (Task 4): the pact routes — offer/accept/decline. All three share
// Plan E Task 1's exact skeleton (`lc_lock` -> `load_lc` -> mutate -> `map_lc`
// on error -> persist -> `204`) and, like arm/disarm/target, publish
// `LcTick` alone (tick-only — E5's rule applied a second time): nothing
// `offer_pact`/`accept_pact`/`decline_pact` ever changes is legible on any
// public surface — `pacts`/`pact_offers`/`pact_barred` are never projected
// by `public_view()` (G13), so a full re-render/re-broadcast would carry no
// public information at all, only free the market's private state to a
// spectator who has no business seeing a market exists. But both parties'
// own phones still need the private re-fetch signal to repaint their own
// `#lc-pacts` section, and the spectator screen never notices: it consumes
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

#[derive(Deserialize)]
pub struct PactOfferForm {
    pub target: usize,
}

#[derive(Deserialize)]
pub struct PactFromForm {
    pub from: usize,
}

// -------------------------------------------------------------
// Plan E (Task 2): the beat clock — a persisted deadline field, the
// auto-beat advance chain, the 1 Hz ticker, and the begin route. Timer state
// is DATA: `beat_deadline_ms` is written and read only here (and in
// `mechanics::tick`, which just calls through to `lc_tick_room`) — the
// engine (`last_call.rs`) never calls a clock function.
// -------------------------------------------------------------

/// Unix ms, used both to arm deadlines and to check them. The single clock
/// read every route/ticker call goes through.
pub(crate) fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock before 1970")
        .as_millis() as i64
}

/// Decision E1/E2: round 1's Draw is the untimed registration lobby (a timer
/// there would advance past set_vessel's Draw gate before anyone
/// registered); every other beat takes its DDv2 §5 duration or stays
/// untimed (auto beats).
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
pub(crate) fn lc_advance_chain(st: &mut LastCallState, now: i64) {
    if st.players.is_empty() {
        return; // M3: nothing to advance; resolve() no-ops here and would spin
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
    arm_beat_clock(st, now);
}

/// The Last Call beat clock, ridden on mechanics.rs's global 1 Hz ticker
/// (decision E16). Advisory pre-check WITHOUT the lock first — one indexed
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
    lc_advance_chain(&mut st, now_ms());
    let ctx = LcCtx { room, game, st };
    persist_and_broadcast_lc(state, &ctx).await;
}

/// `POST /room/{code}/lastcall/begin` — starts round 1's timed loop. Any
/// member may press it, the same `tm_roll_handler` any-member precedent (no
/// notion of "whose turn to begin" exists at the registration lobby). Refuses
/// off round 1's Draw (already begun, or — defensively — a state this route
/// should never see off the lobby) and refuses under two registered players
/// (`vessels.is_empty()` is "hasn't set a drink yet", the same test
/// `lc_start_handler`'s member-count gate uses one level up, but here it's
/// "registered", not merely "seated" — a member can join the room and sit
/// without ever calling `/vessel`). On success: Draw -> Deal (auto) ->
/// Diplomacy, 60s armed.
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
    lc_advance_chain(&mut ctx.st, now_ms());
    persist_and_broadcast_lc(&state, &ctx).await;
    StatusCode::NO_CONTENT.into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 3 players with vessels, round bumped to 2 so Draw is timed (E1 only
    /// exempts round 1). Walks the whole timed-beat chain, asserting the
    /// exact deadline at each stop plus that Deal never surfaces as a
    /// separate stop (E4: `lc_advance_chain` collapses it in the same pass
    /// as the user-visible advance).
    #[test]
    fn test_advance_chain_walks_timed_beats_and_skips_auto_ones() {
        let mut st = LastCallState::new(vec![(1, "a".into()), (2, "b".into()), (3, "c".into())], 1);
        st.set_vessel(1, Deck::Beer, "can").unwrap();
        st.set_vessel(2, Deck::Cider, "bottle").unwrap();
        st.set_vessel(3, Deck::Wine, "glass").unwrap();
        st.round = 2;

        let now = 1_000_000;
        lc_advance_chain(&mut st, now);
        assert_eq!(st.beat, Beat::Diplomacy, "Deal must be skipped");
        assert_eq!(st.beat_deadline_ms, Some(now + 60_000));

        let now = 2_000_000;
        lc_advance_chain(&mut st, now);
        assert_eq!(st.beat, Beat::Lock);
        assert_eq!(st.beat_deadline_ms, Some(now + 45_000));

        let now = 3_000_000;
        lc_advance_chain(&mut st, now);
        assert_eq!(st.beat, Beat::Reveal);
        assert_eq!(st.beat_deadline_ms, Some(now + 20_000));

        // From Reveal: advance_beat (-> Resolve), then the loop's own
        // resolve() branch rolls the round over.
        let now = 4_000_000;
        lc_advance_chain(&mut st, now);
        assert_eq!(st.round, 3);
        assert_eq!(st.beat, Beat::Draw);
        assert_eq!(
            st.beat_deadline_ms,
            Some(now + 30_000),
            "round >= 2 Draw is timed"
        );
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

        lc_advance_chain(&mut st, 1_000_000); // Lock -> Reveal
        assert_eq!(st.beat, Beat::Reveal);
        lc_advance_chain(&mut st, 2_000_000); // Reveal -> advance(Resolve) -> resolve()

        assert_eq!(st.outcome(), Some(crate::last_call::LcOutcome::Winner(0)));
        assert_eq!(st.beat, Beat::Resolve);
        assert_eq!(st.beat_deadline_ms, None);
    }

    /// E1: a fresh state's round-1 Draw stays untimed; the same state moved
    /// to round 2 arms the DRAW_SECS deadline.
    #[test]
    fn test_round_one_draw_is_untimed() {
        let mut st = LastCallState::new(vec![(1, "a".into()), (2, "b".into())], 1);
        arm_beat_clock(&mut st, 1_000_000);
        assert_eq!(st.beat_deadline_ms, None);

        st.round = 2;
        arm_beat_clock(&mut st, 1_000_000);
        assert_eq!(st.beat_deadline_ms, Some(1_000_000 + 30_000));
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
            lc_advance_chain(&mut st, 1_000_000);
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
        lc_advance_chain(&mut route_st, now_ms());
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
        // round+1 Draw — and the deadline the ticker's recheck saw is the
        // route's freshly-armed one, still in the future: proof the ticker
        // no-opped rather than double-advancing.
        assert_eq!(after.beat, Beat::Reveal);
        assert!(after.beat_deadline_ms.is_some_and(|d| d > now_ms()));
    }

    /// alice(1)/bob(2)/cara(3)/dave(4) -> seats 0-3, the same shape as
    /// `last_call.rs`'s own `at_diplomacy()` test fixture — private to that
    /// module, so redefined here rather than reached across the crate.
    /// §7.8: `pacts_section_html` is structure + `data-*` only — no
    /// `hx-post`, no `onclick`, no `action=`. `data-lc-post`/`data-lc-body`
    /// are data-contract attributes (the same status as Plan E's
    /// `data-lc-post`), not behaviour, so neither is in this banned list.
    fn assert_no_behaviour(html: &str) {
        for banned in ["hx-post", "hx-get", "onclick", "action=\""] {
            assert!(
                !html.contains(banned),
                "found forbidden `{banned}` in: {html}"
            );
        }
    }

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

    #[test]
    fn test_pacts_section_states() {
        let mut st = diplomacy_state();

        // Fresh state, viewer alice: 3 PROPOSE TO buttons, no ACCEPT, no
        // "PACT WITH", no WAITING.
        let alice_html = pacts_section_html(&st, 1);
        assert_eq!(alice_html.matches("lc-pact-propose").count(), 3);
        assert!(alice_html
            .contains(r#"data-lc-post="pact/offer" data-lc-body="target=1">PROPOSE TO BOB"#));
        assert!(alice_html
            .contains(r#"data-lc-post="pact/offer" data-lc-body="target=2">PROPOSE TO CARA"#));
        assert!(alice_html
            .contains(r#"data-lc-post="pact/offer" data-lc-body="target=3">PROPOSE TO DAVE"#));
        assert!(!alice_html.contains("ACCEPT"));
        assert!(!alice_html.contains("PACT WITH"));
        assert!(!alice_html.contains("WAITING"));
        assert_no_behaviour(&alice_html);

        // Captured before the offer below, for the third-party-invisibility
        // check.
        let cara_before = pacts_section_html(&st, 3);

        st.offer_pact(1, 1).unwrap(); // alice (seat 0) -> bob (seat 1)

        let alice_html = pacts_section_html(&st, 1);
        assert!(alice_html.contains("OFFERED TO BOB — WAITING"));
        assert_eq!(alice_html.matches("lc-pact-propose").count(), 2);
        assert!(alice_html.contains("PROPOSE TO CARA"));
        assert!(alice_html.contains("PROPOSE TO DAVE"));
        assert!(!alice_html.contains("PROPOSE TO BOB"));

        let bob_html = pacts_section_html(&st, 2);
        assert!(bob_html.contains("ALICE OFFERS A PACT"));
        assert!(bob_html.contains(r#"data-lc-post="pact/accept" data-lc-body="from=0""#));
        assert!(bob_html.contains(r#"data-lc-post="pact/decline" data-lc-body="from=0""#));
        assert_no_behaviour(&bob_html);

        // cara: offers between third parties are invisible — her section is
        // byte-identical to before the offer.
        let cara_after = pacts_section_html(&st, 3);
        assert_eq!(
            cara_after, cara_before,
            "an offer between two other seats must not touch a third party's section"
        );

        st.accept_pact(2, 0).unwrap(); // bob accepts alice's offer

        let alice_html = pacts_section_html(&st, 1);
        assert!(alice_html.contains("PACT WITH BOB — SINCE ROUND 1"));
        assert!(!alice_html.contains("lc-pact-propose"));

        // cara: STILL proposes to alice, bob and dave — secretly-pacted
        // seats stay listed (G11: no pact detector).
        let cara_html = pacts_section_html(&st, 3);
        assert!(cara_html.contains("PROPOSE TO ALICE"));
        assert!(cara_html.contains("PROPOSE TO BOB"));
        assert!(cara_html.contains("PROPOSE TO DAVE"));

        st.pact_barred.push(3); // dave publicly barred

        let dave_html = pacts_section_html(&st, 4);
        assert_eq!(
            dave_html,
            r#"<section class="lc-pacts"><h2>Pact</h2><p class="lc-pact-barred">YOU BROKE A PACT — NOBODY DEALS WITH YOU NOW</p></section>"#
        );

        // cara's propose list drops dave now that he's publicly barred.
        let cara_html = pacts_section_html(&st, 3);
        assert!(!cara_html.contains("PROPOSE TO DAVE"));
        assert!(cara_html.contains("PROPOSE TO ALICE"));
        assert!(cara_html.contains("PROPOSE TO BOB"));

        st.beat = Beat::Lock;
        // Pacted alice keeps the standing line at any beat.
        let alice_html = pacts_section_html(&st, 1);
        assert!(alice_html.contains("PACT WITH BOB — SINCE ROUND 1"));
        // Unpacted cara has nothing to show once the market beat has passed.
        let cara_html = pacts_section_html(&st, 3);
        assert_eq!(cara_html, "");

        st.beat = Beat::Diplomacy;
        st.players[3].status = Status::Eliminated; // dave out: 3 alive < PACT_MIN_ALIVE
        let cara_html = pacts_section_html(&st, 3);
        assert_eq!(cara_html, "");
    }

    #[test]
    fn test_pacts_section_betrayed_notice() {
        let mut st = diplomacy_state();
        st.pact_breaks = vec![crate::last_call::PactBreak {
            betrayer: 0,
            betrayed: 1,
            round: 1,
        }];
        st.beat = Beat::Lock;

        let bob_html = pacts_section_html(&st, 2); // bob is seat 1, the betrayed
        assert!(bob_html.contains("ALICE BROKE YOUR PACT"));

        st.round = 2;
        let bob_html = pacts_section_html(&st, 2);
        assert!(!bob_html.contains("BROKE YOUR PACT"));

        st.round = 1;
        st.pact_barred.push(0); // alice barred, but at Lock the barred line never shows
        let alice_html = pacts_section_html(&st, 1); // alice is seat 0, the betrayer
        assert_eq!(alice_html, "");
    }

    /// Plan G whole-plan-review erratum (originally found by Task 3, fixed
    /// as a cross-task seam repair): `resolve()` used to stamp
    /// `PactBreak { round: self.round }` in Step 1, BEFORE Step 8's
    /// rollover bumps `self.round` in the same call — and since
    /// `lc_advance_chain` never persists an intermediate state between the
    /// two (Reveal -> Resolve -> the round+1 Draw is one synchronous pass),
    /// no client fetch could ever observe `st.round` still equal to the
    /// round a non-terminal betrayal was recorded against. Both round-scoped
    /// readers (`pacts_section_html`'s betrayed notice, `lc_screen_panel`'s
    /// break strip, both filtering `round == st.round`/`round == view.round`
    /// verbatim per the brief) were unreachable for every betrayal except
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

        // Both round-scoped surfaces now show the betrayal in the very
        // first frame anyone can fetch it in.
        assert!(pacts_section_html(&st, 2).contains("ALICE BROKE YOUR PACT"));
        let view = st.public_view();
        assert!(lc_render::lc_screen_panel(&view).contains("lc-pact-break"));

        // One more resolve (no new betrayal) rolls the round over again —
        // the break ages out of both surfaces, per G5 ("loud", not
        // "permanent").
        st.beat = Beat::Resolve;
        st.resolve().unwrap();
        assert_ne!(st.pact_breaks.last().unwrap().round, st.round);
        assert!(!pacts_section_html(&st, 2).contains("BROKE YOUR PACT"));
        assert!(!lc_render::lc_screen_panel(&st.public_view()).contains("lc-pact-break"));
    }
}
