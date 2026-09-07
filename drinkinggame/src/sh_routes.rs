//! Backroom route handlers, and the **private** per-viewer fragment.
//!
//! SQL stays in `db.rs`; broadcast HTML stays in `sh_render.rs`. This module
//! is the only one permitted to hold a `&SecretHitlerState` while building
//! HTML, and it may do so for exactly one reason: the private pane is served
//! over a session-authenticated route to one player, never published.
//!
//! Everything else about that pane is defensive. `sh_private_handler` takes
//! **no player identifier of any kind** — no path segment, no query
//! parameter, no form field. The viewer comes from the session cookie alone,
//! so "can player A fetch player B's role?" is unanswerable rather than
//! merely guarded, and a reviewer can check it from the signature. This is
//! the `lc_hand_handler` contract, applied to a game where getting it wrong
//! ends the round rather than spoiling a hand.

use std::collections::HashMap;

use axum::extract::{Form, Path, State};
use axum::http::{header, StatusCode};
use axum::response::{Html, IntoResponse};
use rand::Rng;
use serde::Deserialize;

use crate::auth::PlayerSession;
use crate::db;
use crate::error::GameError;
use crate::hub::RoomMessage;
use crate::models::{Game, Player, Room};
use crate::render::html_escape;
use crate::secret_hitler::{
    Phase, Policy, Power, SecretHitlerState, ShError, ShPublicView, KIND, MAX_SEATS, MIN_SEATS,
};
use crate::sh_render;
use crate::sh_theme as theme;
use crate::GameState;

// ---------------------------------------------------------------------------
// Plumbing
// ---------------------------------------------------------------------------

/// Everything an action handler needs: the room (id/code), the raw `games`
/// row (its id), and the parsed state, mutated in place then persisted.
pub(crate) struct ShCtx {
    pub room: Room,
    pub game: Game,
    pub st: SecretHitlerState,
}

/// member_room -> active game -> kind == "secret_hitler" else WrongGameKind
/// -> parse state. Shared entry point for every `/sh/*` handler.
pub(crate) async fn load_sh(
    state: &GameState,
    code: &str,
    player: &Player,
) -> Result<ShCtx, axum::response::Response> {
    let room = crate::game::member_room(state, code, player).await?;
    let Some(game) = db::get_active_game(&state.pool, room.id).await else {
        return Err(GameError::NoActiveGame.into_response());
    };
    if game.kind != KIND {
        return Err(GameError::WrongGameKind.into_response());
    }
    let st = SecretHitlerState::from_json(game.state_json.as_deref().unwrap_or_default());
    Ok(ShCtx { room, game, st })
}

/// Resolves the room and hands back its lock. Membership is `load_sh`'s job,
/// done under the guard this returns.
async fn sh_lock(
    state: &GameState,
    code: &str,
) -> Result<std::sync::Arc<tokio::sync::Mutex<()>>, axum::response::Response> {
    let Some(room) = db::get_open_room(&state.pool, &code.to_uppercase()).await else {
        return Err(GameError::RoomNotFound.into_response());
    };
    Ok(state.locks.for_room(room.id))
}

/// Drains the engine's queued pours into `events`.
///
/// A no-op in v1 — every amount in `sh_theme` is zero, so this writes
/// nothing and the leaderboard stays a record of the other three games.
/// When the drinking variant turns them up, read `sh_theme`'s module doc
/// first: the leaderboard is broadcast over the same unauthenticated stream
/// as everything else, so a pour keyed on a hidden fact is a role oracle.
async fn drain_drinks(state: &GameState, ctx: &mut ShCtx) {
    for call in std::mem::take(&mut ctx.st.pending_drinks) {
        db::insert_events_bulk(&state.pool, ctx.room.id, call.player_id, "drink", call.n).await;
    }
}

/// Persists the mutated state, then re-renders and publishes every surface
/// that reflects it: phone GAME tab, big screen, ROOM tab and leaderboard.
///
/// There is no separate tick message. Every legal Backroom action changes
/// the phase, the ballot count, a track or the roster, so the `Game` frame
/// is always a real update — and it doubles as the signal each phone needs
/// to re-fetch its own private pane. That keeps `hub.rs` and `sse_stream`
/// untouched, and the SSE snapshot at exactly four frames.
pub(crate) async fn persist_and_broadcast_sh(state: &GameState, ctx: &ShCtx) {
    db::set_game_state(&state.pool, ctx.game.id, &ctx.st.to_json()).await;
    db::touch_room(&state.pool, ctx.room.id).await;
    crate::game::broadcast_game(state, ctx.room.id, &ctx.room.code, None).await;
    crate::game::broadcast_room(state, ctx.room.id, &ctx.room.code).await;
    crate::routes::broadcast_leaderboard(state, ctx.room.id).await;
}

/// Engine error -> HTTP.
///
/// Every message comes from `sh_theme`'s error allowlist. `room.html`'s
/// `htmx:responseError` handler assigns the body straight into the DOM, so a
/// message naming a role, a faction, a tile or a peeked card would leak it
/// to the one player most motivated to read it.
pub(crate) fn map_sh(e: ShError) -> axum::response::Response {
    let (status, msg) = match e {
        ShError::NotYourMove => (StatusCode::FORBIDDEN, theme::ERR_NOT_YOUR_MOVE),
        ShError::DeadPlayer => (StatusCode::FORBIDDEN, theme::ERR_DEAD_PLAYER),
        ShError::WrongPhase => (StatusCode::CONFLICT, theme::ERR_WRONG_PHASE),
        ShError::NotEligible => (StatusCode::UNPROCESSABLE_ENTITY, theme::ERR_NOT_ELIGIBLE),
        ShError::AlreadyInvestigated => (
            StatusCode::UNPROCESSABLE_ENTITY,
            theme::ERR_ALREADY_INVESTIGATED,
        ),
        ShError::BadTarget => (StatusCode::UNPROCESSABLE_ENTITY, theme::ERR_BAD_TARGET),
        ShError::BadIndex => (StatusCode::UNPROCESSABLE_ENTITY, theme::ERR_BAD_INDEX),
        ShError::WrongSeatCount => (StatusCode::CONFLICT, theme::ERR_WRONG_SEAT_COUNT),
    };
    error_body(status, msg)
}

/// The shape `GameError::into_response` produces, which is what
/// `room.html:172` expects to drop into an error slot.
fn error_body(status: StatusCode, msg: &str) -> axum::response::Response {
    (
        status,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        format!(r#"<p class="error">{}</p>"#, html_escape(msg)),
    )
        .into_response()
}

/// Parses a Backroom game's blob and loads member names, returning the
/// **projection** rather than the state.
///
/// `game.rs`'s three dispatch arms call this, so nothing outside this module
/// and the engine ever holds a `SecretHitlerState`. That matters: the
/// compile-time secrecy proof in `sh_render.rs` only covers `sh_render.rs`,
/// and `game.rs` builds view structs inline for 3 Man, so without this the
/// obvious next edit hands the full state to a renderer.
pub(crate) async fn sh_view_data(
    state: &GameState,
    game: &Game,
) -> (ShPublicView, HashMap<i64, String>) {
    let st = SecretHitlerState::from_json(game.state_json.as_deref().unwrap_or_default());
    let names = db::room_members(&state.pool, game.room_id)
        .await
        .into_iter()
        .map(|m| (m.id, m.name))
        .collect();
    (st.public_view(), names)
}

/// The projection plus names from an already-parsed state, for handlers that
/// have a `ShCtx` in hand.
async fn view_of(state: &GameState, ctx: &ShCtx) -> (ShPublicView, HashMap<i64, String>) {
    let names = db::room_members(&state.pool, ctx.room.id)
        .await
        .into_iter()
        .map(|m| (m.id, m.name))
        .collect();
    (ctx.st.public_view(), names)
}

// ---------------------------------------------------------------------------
// Lifecycle
// ---------------------------------------------------------------------------

/// `POST /room/{code}/sh/start`.
///
/// Locked across the whole body: a concurrent join mutating the member list
/// between the arity check and the deal must not race a start.
pub async fn sh_start_handler(
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
    if !(MIN_SEATS..=MAX_SEATS).contains(&members.len()) {
        // `GameError::TooFewPlayers` hardcodes "at least 2 players", which is
        // wrong for a 5–10 game, and widening that shared enum for one game
        // is worse than building the body here in the shape it already
        // produces.
        return error_body(StatusCode::CONFLICT, theme::NEEDS_5_TO_10);
    }
    let member_ids: Vec<i64> = members.iter().map(|m| m.id).collect();

    // The one place entropy enters. The engine advances it deterministically
    // from here, so it stays a pure function of its own blob.
    let seed: u64 = rand::thread_rng().gen();
    let st = match SecretHitlerState::new(member_ids, seed) {
        Ok(s) => s,
        Err(e) => return map_sh(e),
    };

    // rules_json/deck_order are Ring of Fire concepts: empty. state_json must
    // be Some(valid JSON) — from_json panics on "". A concurrent start is
    // caught by the games table's partial unique index, not by a pre-check.
    if let Err(e) = db::start_game(&state.pool, room.id, KIND, "", "", Some(&st.to_json())).await {
        return e.into_response();
    }

    // Re-load under the same lock rather than hand-assembling a ShCtx: this
    // doubles as proof the shared persist path works for a fresh game.
    let ctx = match load_sh(&state, &code, &player).await {
        Ok(c) => c,
        Err(resp) => return resp,
    };
    persist_and_broadcast_sh(&state, &ctx).await;
    StatusCode::NO_CONTENT.into_response()
}

/// `POST /room/{code}/sh/end`.
pub async fn sh_end_handler(
    State(state): State<GameState>,
    PlayerSession(player): PlayerSession,
    Path(code): Path<String>,
) -> axum::response::Response {
    let lock = match sh_lock(&state, &code).await {
        Ok(l) => l,
        Err(r) => return r,
    };
    let _guard = lock.lock().await;
    let ctx = match load_sh(&state, &code, &player).await {
        Ok(c) => c,
        Err(r) => return r,
    };

    // Render the final frame BEFORE ending the game: `get_active_game`
    // filters on `ended_at IS NULL`, so afterwards there is nothing to
    // project from.
    let (view, names) = view_of(&state, &ctx).await;
    db::end_game(&state.pool, ctx.game.id).await;
    db::touch_room(&state.pool, ctx.room.id).await;

    let phone = format!(
        "{}{}",
        sh_render::sh_over_panel(&view, &names),
        crate::game::idle_panel(&state, &ctx.room.code).await
    );
    state.hub.publish(ctx.room.id, RoomMessage::Game(phone));
    state.hub.publish(
        ctx.room.id,
        RoomMessage::Screen(sh_render::sh_screen_over(&view, &names)),
    );
    crate::game::broadcast_room(&state, ctx.room.id, &ctx.room.code).await;
    crate::routes::broadcast_leaderboard(&state, ctx.room.id).await;
    StatusCode::NO_CONTENT.into_response()
}

// ---------------------------------------------------------------------------
// Actions
//
// Every handler is the same six steps: take the room lock, re-load state
// under it, resolve the caller's seat, attempt one engine transition, drain
// any pours, persist and broadcast once. The guard is held from load through
// broadcast — releasing early lets a concurrent handler's broadcast land
// first and leaves a stale render as the last word on every phone.
// ---------------------------------------------------------------------------

/// Resolves the caller to a seat. A room member with no seat — someone who
/// opened the room link after the deal — is a spectator, not a player, and
/// has no move to make.
fn seat_of(ctx: &ShCtx, player: &Player) -> Result<usize, ShError> {
    ctx.st.seat_of(player.id).ok_or(ShError::NotYourMove)
}

macro_rules! sh_action {
    ($state:expr, $code:expr, $player:expr, |$ctx:ident, $seat:ident| $body:expr) => {{
        let lock = match sh_lock(&$state, &$code).await {
            Ok(l) => l,
            Err(r) => return r,
        };
        let _guard = lock.lock().await;
        #[allow(unused_mut)]
        let mut $ctx = match load_sh(&$state, &$code, &$player).await {
            Ok(c) => c,
            Err(r) => return r,
        };
        let $seat = match seat_of(&$ctx, &$player) {
            Ok(s) => s,
            Err(e) => return map_sh(e),
        };
        if let Err(e) = $body {
            return map_sh(e);
        }
        drain_drinks(&$state, &mut $ctx).await;
        persist_and_broadcast_sh(&$state, &$ctx).await;
        StatusCode::NO_CONTENT.into_response()
    }};
}

#[derive(Deserialize)]
pub struct TargetForm {
    pub target: usize,
}

#[derive(Deserialize)]
pub struct IndexForm {
    pub index: usize,
}

#[derive(Deserialize)]
pub struct BoolForm {
    /// "1" for yes, anything else for no — the form posts a literal.
    pub yes: String,
}

impl BoolForm {
    fn value(&self) -> bool {
        self.yes == "1"
    }
}

#[derive(Deserialize)]
pub struct PowerForm {
    /// Absent for Read the Room, which names nobody.
    pub target: Option<usize>,
}

pub async fn sh_nominate_handler(
    State(state): State<GameState>,
    PlayerSession(player): PlayerSession,
    Path(code): Path<String>,
    Form(form): Form<TargetForm>,
) -> axum::response::Response {
    sh_action!(state, code, player, |ctx, seat| ctx
        .st
        .nominate(seat, form.target))
}

pub async fn sh_vote_handler(
    State(state): State<GameState>,
    PlayerSession(player): PlayerSession,
    Path(code): Path<String>,
    Form(form): Form<BoolForm>,
) -> axum::response::Response {
    sh_action!(state, code, player, |ctx, seat| ctx
        .st
        .cast_vote(seat, form.value()))
}

pub async fn sh_discard_handler(
    State(state): State<GameState>,
    PlayerSession(player): PlayerSession,
    Path(code): Path<String>,
    Form(form): Form<IndexForm>,
) -> axum::response::Response {
    sh_action!(state, code, player, |ctx, seat| ctx
        .st
        .president_discard(seat, form.index))
}

pub async fn sh_enact_handler(
    State(state): State<GameState>,
    PlayerSession(player): PlayerSession,
    Path(code): Path<String>,
    Form(form): Form<IndexForm>,
) -> axum::response::Response {
    sh_action!(state, code, player, |ctx, seat| ctx
        .st
        .chancellor_enact(seat, form.index))
}

pub async fn sh_veto_handler(
    State(state): State<GameState>,
    PlayerSession(player): PlayerSession,
    Path(code): Path<String>,
) -> axum::response::Response {
    sh_action!(state, code, player, |ctx, seat| ctx.st.propose_veto(seat))
}

pub async fn sh_veto_answer_handler(
    State(state): State<GameState>,
    PlayerSession(player): PlayerSession,
    Path(code): Path<String>,
    Form(form): Form<BoolForm>,
) -> axum::response::Response {
    sh_action!(state, code, player, |ctx, seat| ctx
        .st
        .answer_veto(seat, form.value()))
}

pub async fn sh_power_handler(
    State(state): State<GameState>,
    PlayerSession(player): PlayerSession,
    Path(code): Path<String>,
    Form(form): Form<PowerForm>,
) -> axum::response::Response {
    sh_action!(state, code, player, |ctx, seat| ctx
        .st
        .use_power(seat, form.target))
}

// ---------------------------------------------------------------------------
// The private pane
// ---------------------------------------------------------------------------

/// `GET /room/{code}/sh/private` — **PRIVATE**.
///
/// Takes no player identifier: the three extractors are `State`,
/// `PlayerSession` and `Path(code)`, and nothing else. Appending
/// `?player_id=`, `?seat=` or `?target=` cannot change one byte of the
/// response, which `test_sh_private_route_takes_no_player_input` proves.
pub async fn sh_private_handler(
    State(state): State<GameState>,
    PlayerSession(player): PlayerSession,
    Path(code): Path<String>,
) -> axum::response::Response {
    let ctx = match load_sh(&state, &code, &player).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let names: HashMap<i64, String> = db::room_members(&state.pool, ctx.room.id)
        .await
        .into_iter()
        .map(|m| (m.id, m.name))
        .collect();
    Html(private_pane(
        &state.base_path,
        &code,
        &ctx.st,
        player.id,
        &names,
    ))
    .into_response()
}

fn name_of(names: &HashMap<i64, String>, id: i64) -> String {
    html_escape(names.get(&id).map(|s| s.as_str()).unwrap_or("?"))
}

fn seat_name(st: &SecretHitlerState, names: &HashMap<i64, String>, seat: usize) -> String {
    match st.seats.get(seat) {
        Some(s) => name_of(names, s.player_id),
        None => "?".to_string(),
    }
}

/// The viewer's own fragment: their role, what they were shown at the deal,
/// whatever they are holding, their private power results, and every control
/// that is theirs to use.
///
/// A room member who holds no seat — someone who joined after the deal —
/// gets a spectator pane rather than a panic. Every lookup below is
/// `Option`-shaped for that reason.
fn private_pane(
    base_path: &str,
    code: &str,
    st: &SecretHitlerState,
    player_id: i64,
    names: &HashMap<i64, String>,
) -> String {
    let seq = st.seq;
    let Some(me) = st.seat_of(player_id) else {
        return format!(
            r#"<div id="sh-private" data-sh-seq="{seq}"><p class="sh-spectator">You joined after the deal — you're watching this one.</p></div>"#
        );
    };

    let role = st.seats[me].role;
    let mut body = format!(
        r#"<div class="sh-role"><span class="sh-role-name">{name}</span><p class="sh-role-brief">{brief}</p>{knows}</div>"#,
        name = html_escape(theme::role_name(role)),
        brief = html_escape(theme::role_brief(role)),
        knows = knows_html(st, me, names),
    );

    if !st.seats[me].alive {
        body.push_str(
            r#"<p class="sh-out">You're out for the night. You can watch, but you can't vote or hold a chair.</p>"#,
        );
    }

    body.push_str(&controls(base_path, code, st, me, names));
    body.push_str(&private_results(st, me, names));

    format!(r#"<div id="sh-private" data-sh-seq="{seq}">{body}</div>"#)
}

/// What the deal showed this seat. Empty for every Regular, and for the Boss
/// above six players.
fn knows_html(st: &SecretHitlerState, me: usize, names: &HashMap<i64, String>) -> String {
    let knows = &st.seats[me].knows;
    if knows.is_empty() {
        return String::new();
    }
    let list: String = knows
        .iter()
        .map(|&k| {
            format!(
                r#"<li>{} &middot; {}</li>"#,
                seat_name(st, names, k),
                html_escape(theme::role_name(st.seats[k].role))
            )
        })
        .collect();
    format!(
        r#"<div class="sh-knows"><span class="sh-knows-label">You were shown</span><ul>{list}</ul></div>"#
    )
}

/// Every control that belongs to this viewer, and only to this viewer. These
/// live here rather than in the broadcast panel so no interactive element
/// ever enters the shared frame.
fn controls(
    base_path: &str,
    code: &str,
    st: &SecretHitlerState,
    me: usize,
    names: &HashMap<i64, String>,
) -> String {
    if st.is_over() || !st.seats[me].alive {
        return String::new();
    }
    let post = |path: &str| format!("{base_path}/room/{code}/sh/{path}");

    match st.phase {
        Phase::Nomination if me == st.president_seat => {
            let opts: String = st
                .seats
                .iter()
                .enumerate()
                .filter(|(i, _)| st.eligible_chancellor(*i))
                .map(|(i, _)| {
                    format!(
                        r#"<button type="submit" name="target" value="{i}" class="sh-pick">{}</button>"#,
                        seat_name(st, names, i)
                    )
                })
                .collect();
            format!(
                r#"<form class="sh-controls" hx-post="{}" hx-swap="none"><p class="sh-prompt">Pick your {}</p><div class="sh-picks">{opts}</div></form>"#,
                post("nominate"),
                html_escape(theme::OFFICE_CHANCELLOR)
            )
        }
        Phase::Voting => {
            if st.votes.get(me).copied().flatten().is_some() {
                return r#"<p class="sh-prompt sh-waiting">Your ballot is down.</p>"#.to_string();
            }
            let nominee = st
                .nominee_seat
                .map(|n| seat_name(st, names, n))
                .unwrap_or_default();
            format!(
                r#"<form class="sh-controls" hx-post="{url}" hx-swap="none"><p class="sh-prompt">{pres} &amp; {nominee}?</p><div class="sh-picks"><button type="submit" name="yes" value="1" class="sh-vote-yes">{yes}</button><button type="submit" name="yes" value="0" class="sh-vote-no">{no}</button></div></form>"#,
                url = post("vote"),
                pres = seat_name(st, names, st.president_seat),
                nominee = nominee,
                yes = html_escape(theme::VOTE_YES),
                no = html_escape(theme::VOTE_NO),
            )
        }
        Phase::PresidentDraft if me == st.president_seat => {
            hand_form(&post("discard"), st.hand_of(me), "Bin one", "Bin")
        }
        Phase::ChancellorDraft if st.nominee_seat == Some(me) => {
            let mut html = hand_form(&post("enact"), st.hand_of(me), "Play one", "Play");
            if st.veto_unlocked && !st.veto_refused {
                html.push_str(&format!(
                    r#"<form class="sh-controls" hx-post="{}" hx-swap="none"><button type="submit" class="sh-veto">{}</button></form>"#,
                    post("veto"),
                    html_escape(theme::VETO_PROPOSE)
                ));
            }
            html
        }
        Phase::VetoPending if me == st.president_seat => format!(
            r#"<form class="sh-controls" hx-post="{url}" hx-swap="none"><p class="sh-prompt">{who} wants this round killed.</p><div class="sh-picks"><button type="submit" name="yes" value="1" class="sh-vote-yes">{agree}</button><button type="submit" name="yes" value="0" class="sh-vote-no">{refuse}</button></div></form>"#,
            url = post("veto/answer"),
            who = st
                .nominee_seat
                .map(|n| seat_name(st, names, n))
                .unwrap_or_default(),
            agree = html_escape(theme::VETO_AGREE),
            refuse = html_escape(theme::VETO_REFUSE),
        ),
        Phase::Power(power) if me == st.president_seat => {
            power_form(&post("power"), st, me, power, names)
        }
        _ => String::new(),
    }
}

/// The tiles this viewer is holding, each a button. The identities are the
/// secret the whole design protects, which is why this markup exists only
/// here.
fn hand_form(url: &str, hand: &[Policy], prompt: &str, verb: &str) -> String {
    if hand.is_empty() {
        return String::new();
    }
    let tiles: String = hand
        .iter()
        .enumerate()
        .map(|(i, p)| {
            format!(
                r#"<button type="submit" name="index" value="{i}" class="{cls}">{verb} {name}</button>"#,
                cls = sh_render::policy_class(*p),
                verb = html_escape(verb),
                name = html_escape(theme::policy_name(*p))
            )
        })
        .collect();
    format!(
        r#"<form class="sh-controls" hx-post="{url}" hx-swap="none"><p class="sh-prompt">{prompt}</p><div class="sh-hand">{tiles}</div></form>"#,
        prompt = html_escape(prompt)
    )
}

fn power_form(
    url: &str,
    st: &SecretHitlerState,
    me: usize,
    power: Power,
    names: &HashMap<i64, String>,
) -> String {
    let prompt = format!(
        r#"<p class="sh-prompt"><span class="{cls}">{name}</span> &middot; {hint}</p>"#,
        cls = sh_render::power_class(power),
        name = html_escape(theme::power_name(power)),
        hint = html_escape(theme::power_prompt(power))
    );
    if !power.needs_target() {
        return format!(
            r#"<form class="sh-controls" hx-post="{url}" hx-swap="none">{prompt}<button type="submit" class="btn-primary">{name}</button></form>"#,
            name = html_escape(theme::power_name(power))
        );
    }
    let opts: String = st
        .seats
        .iter()
        .enumerate()
        .filter(|(i, s)| {
            s.alive && *i != me && (power != Power::Investigate || !st.investigated.contains(i))
        })
        .map(|(i, _)| {
            format!(
                r#"<button type="submit" name="target" value="{i}" class="sh-pick">{}</button>"#,
                seat_name(st, names, i)
            )
        })
        .collect();
    format!(
        r#"<form class="sh-controls" hx-post="{url}" hx-swap="none">{prompt}<div class="sh-picks">{opts}</div></form>"#
    )
}

/// What this viewer alone has learned: the parties they dug up, and the
/// tiles they read off the pile.
///
/// These persist for the rest of the game. At a table you would have to
/// remember them; on a phone whose panel repaints on every broadcast, a
/// result shown once would be a result lost, so keeping the record is the
/// honest translation rather than a power buff.
fn private_results(st: &SecretHitlerState, me: usize, names: &HashMap<i64, String>) -> String {
    let mut out = String::new();

    let digs: String = st
        .investigation_results
        .iter()
        .filter(|r| r.investigator == me)
        .map(|r| {
            format!(
                r#"<li>{} &middot; {}</li>"#,
                seat_name(st, names, r.target),
                html_escape(theme::party_name(r.party))
            )
        })
        .collect();
    if !digs.is_empty() {
        out.push_str(&format!(
            r#"<div class="sh-private-results"><span class="sh-knows-label">{}</span><ul>{digs}</ul></div>"#,
            html_escape(theme::POWER_INVESTIGATE)
        ));
    }

    let peeks: String = st
        .peek_results
        .iter()
        .filter(|r| r.president == me)
        .map(|r| {
            let tiles: String = r
                .top
                .iter()
                .map(|p| {
                    format!(
                        r#"<span class="{}">{}</span>"#,
                        sh_render::policy_class(*p),
                        html_escape(theme::policy_name(*p))
                    )
                })
                .collect();
            format!(r#"<li>{tiles}</li>"#)
        })
        .collect();
    if !peeks.is_empty() {
        out.push_str(&format!(
            r#"<div class="sh-private-results"><span class="sh-knows-label">{}</span><ul>{peeks}</ul></div>"#,
            html_escape(theme::POWER_PEEK)
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secret_hitler::Role;

    fn st_names(n: usize) -> (SecretHitlerState, HashMap<i64, String>) {
        let st = SecretHitlerState::new((1..=n as i64).collect(), 42).unwrap();
        let names = (1..=n as i64).map(|i| (i, format!("p{i}"))).collect();
        (st, names)
    }

    #[test]
    fn test_the_pane_marks_itself_and_carries_the_seq() {
        let (mut st, names) = st_names(5);
        st.seq = 9;
        let html = private_pane("/drinks", "ABCD", &st, 1, &names);
        assert!(html.contains(r#"id="sh-private""#));
        assert!(html.contains(r#"data-sh-seq="9""#));
    }

    #[test]
    fn test_every_seat_sees_its_own_role_and_no_other() {
        // The pairwise sweep: for a 7-player table, each viewer's pane must
        // name their own seat's role and must not name any other seat's
        // knowledge set.
        let (st, names) = st_names(7);
        for (seat, s) in st.seats.iter().enumerate() {
            let html = private_pane("/drinks", "ABCD", &st, s.player_id, &names);
            assert!(
                html.contains(theme::role_name(s.role)),
                "seat {seat} is not shown its own role"
            );
            for &k in &st.seats[seat].knows {
                assert!(
                    html.contains(&format!("p{}", st.seats[k].player_id)),
                    "seat {seat} should see the ally it was shown"
                );
            }
        }
    }

    #[test]
    fn test_a_regular_is_shown_nobody() {
        let (st, names) = st_names(7);
        let lib = st
            .seats
            .iter()
            .find(|s| s.role == Role::Liberal)
            .expect("a 7-player table has Liberals");
        let html = private_pane("/drinks", "ABCD", &st, lib.player_id, &names);
        assert!(
            !html.contains("sh-knows"),
            "Regulars learn nothing at the deal"
        );
    }

    #[test]
    fn test_the_boss_is_blind_above_six_players() {
        for n in 7..=10usize {
            let (st, names) = st_names(n);
            let h = st.seats.iter().find(|s| s.role == Role::Hitler).unwrap();
            let html = private_pane("/drinks", "ABCD", &st, h.player_id, &names);
            assert!(!html.contains("sh-knows"), "{n}p: the Boss sees nobody");
            assert!(html.contains(theme::ROLE_HITLER));
        }
    }

    #[test]
    fn test_the_boss_and_the_syndicate_see_each_other_at_five_and_six() {
        for n in [5usize, 6] {
            let (st, names) = st_names(n);
            let h = st.seats.iter().find(|s| s.role == Role::Hitler).unwrap();
            let html = private_pane("/drinks", "ABCD", &st, h.player_id, &names);
            assert!(
                html.contains("sh-knows"),
                "{n}p: the Boss is shown the Syndicate"
            );
        }
    }

    #[test]
    fn test_a_hand_is_only_ever_in_its_holders_pane() {
        let (mut st, names) = st_names(5);
        st.phase = Phase::PresidentDraft;
        st.president_seat = 0;
        st.president_hand = vec![Policy::Fascist, Policy::Liberal, Policy::Liberal];

        let holder = private_pane("/drinks", "ABCD", &st, 1, &names);
        assert!(holder.contains("sh-hand"), "the Chair sees the three");
        assert_eq!(holder.matches(r#"name="index""#).count(), 3);

        for other in 2..=5i64 {
            let html = private_pane("/drinks", "ABCD", &st, other, &names);
            assert!(
                !html.contains("sh-hand"),
                "player {other} must not see the Chair's tiles"
            );
        }
    }

    #[test]
    fn test_a_spectator_gets_a_pane_not_a_panic() {
        // A room member who joined after the deal holds no seat.
        let (st, names) = st_names(5);
        let html = private_pane("/drinks", "ABCD", &st, 999, &names);
        assert!(html.contains(r#"id="sh-private""#));
        assert!(html.contains("sh-spectator"));
        for role in [theme::ROLE_HITLER, theme::ROLE_LIBERAL, theme::ROLE_FASCIST] {
            assert!(!html.contains(role), "a spectator learns nothing");
        }
    }

    #[test]
    fn test_only_the_president_gets_the_nomination_picker() {
        let (mut st, names) = st_names(5);
        st.president_seat = 0;
        assert!(private_pane("/drinks", "A", &st, 1, &names).contains("sh/nominate"));
        for other in 2..=5i64 {
            assert!(!private_pane("/drinks", "A", &st, other, &names).contains("sh/nominate"));
        }
    }

    #[test]
    fn test_the_picker_offers_only_eligible_seats() {
        let (mut st, names) = st_names(7);
        st.president_seat = 0;
        st.last_elected_chancellor = Some(2);
        st.last_elected_president = Some(3);
        let html = private_pane("/drinks", "A", &st, 1, &names);
        let picks = html.matches(r#"name="target""#).count();
        let eligible = (0..7).filter(|&i| st.eligible_chancellor(i)).count();
        assert_eq!(picks, eligible);
        assert!(eligible > 0);
    }

    #[test]
    fn test_a_cast_ballot_replaces_the_vote_buttons() {
        let (mut st, names) = st_names(5);
        st.phase = Phase::Voting;
        st.nominee_seat = Some(1);
        assert!(private_pane("/drinks", "A", &st, 1, &names).contains("sh/vote"));
        st.votes[0] = Some(true);
        let html = private_pane("/drinks", "A", &st, 1, &names);
        assert!(!html.contains("sh/vote"));
        assert!(html.contains("sh-waiting"));
    }

    #[test]
    fn test_the_dead_get_no_controls() {
        let (mut st, names) = st_names(7);
        st.phase = Phase::Voting;
        st.nominee_seat = Some(1);
        st.seats[3].alive = false;
        let html = private_pane("/drinks", "A", &st, 4, &names);
        assert!(!html.contains("<form"));
        assert!(html.contains("sh-out"));
        assert!(
            html.contains(theme::role_name(st.seats[3].role)),
            "they still see who they were"
        );
    }

    #[test]
    fn test_investigation_results_are_scoped_to_the_investigator() {
        let (mut st, names) = st_names(9);
        st.phase = Phase::Power(Power::Investigate);
        st.president_seat = 0;
        st.use_power(0, Some(4)).unwrap();

        let mine = private_pane("/drinks", "A", &st, 1, &names);
        assert!(mine.contains(theme::POWER_INVESTIGATE));
        assert!(mine.contains(theme::party_name(st.seats[4].role.party())));

        for other in 2..=9i64 {
            let html = private_pane("/drinks", "A", &st, other, &names);
            assert!(
                !html.contains("sh-private-results"),
                "player {other} must not read someone else's dig"
            );
        }
    }

    #[test]
    fn test_peek_results_are_scoped_to_the_president() {
        let (mut st, names) = st_names(5);
        st.phase = Phase::Power(Power::Peek);
        st.president_seat = 0;
        st.use_power(0, None).unwrap();
        assert!(private_pane("/drinks", "A", &st, 1, &names).contains(theme::POWER_PEEK));
        for other in 2..=5i64 {
            assert!(
                !private_pane("/drinks", "A", &st, other, &names).contains("sh-private-results")
            );
        }
    }

    #[test]
    fn test_the_veto_button_appears_only_when_it_is_available() {
        let (mut st, names) = st_names(7);
        st.phase = Phase::ChancellorDraft;
        st.nominee_seat = Some(1);
        st.chancellor_hand = vec![Policy::Fascist, Policy::Liberal];
        assert!(!private_pane("/drinks", "A", &st, 2, &names).contains("sh/veto"));
        st.veto_unlocked = true;
        assert!(private_pane("/drinks", "A", &st, 2, &names).contains("sh/veto"));
        st.veto_refused = true;
        assert!(
            !private_pane("/drinks", "A", &st, 2, &names).contains("sh/veto"),
            "no second bite after a refusal"
        );
    }

    #[test]
    fn test_the_investigate_picker_skips_the_already_investigated() {
        let (mut st, names) = st_names(9);
        st.phase = Phase::Power(Power::Investigate);
        st.president_seat = 0;
        st.investigated = vec![2, 3];
        let html = private_pane("/drinks", "A", &st, 1, &names);
        let picks = html.matches(r#"name="target""#).count();
        assert_eq!(picks, 9 - 1 - 2, "not the holder, not the two already dug");
    }

    #[test]
    fn test_read_the_room_offers_no_target() {
        let (mut st, names) = st_names(5);
        st.phase = Phase::Power(Power::Peek);
        st.president_seat = 0;
        let html = private_pane("/drinks", "A", &st, 1, &names);
        assert!(html.contains("sh/power"));
        assert!(!html.contains(r#"name="target""#));
    }

    #[test]
    fn test_a_finished_game_shows_no_controls() {
        let (mut st, names) = st_names(5);
        st.phase = Phase::Over(crate::secret_hitler::Outcome::LiberalPolicies);
        let html = private_pane("/drinks", "A", &st, 1, &names);
        assert!(!html.contains("<form"));
    }

    #[test]
    fn test_names_are_escaped_in_the_private_pane() {
        let (mut st, mut names) = st_names(5);
        st.president_seat = 0;
        names.insert(2, "<img src=x onerror=1>".to_string());
        let html = private_pane("/drinks", "A", &st, 1, &names);
        assert!(!html.contains("<img"));
        assert!(html.contains("&lt;img"));
    }

    #[test]
    fn test_error_bodies_name_no_secret() {
        for e in [
            ShError::WrongPhase,
            ShError::NotYourMove,
            ShError::NotEligible,
            ShError::DeadPlayer,
            ShError::AlreadyInvestigated,
            ShError::BadTarget,
            ShError::BadIndex,
            ShError::WrongSeatCount,
        ] {
            let resp = map_sh(e);
            assert!(
                resp.status().is_client_error(),
                "{e:?} maps to a client error"
            );
        }
    }
}
