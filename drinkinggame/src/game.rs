//! Ring of Fire route handlers and the shared game-panel builder.
//! SQL stays in db.rs; HTML fragments stay in render.rs.

use axum::extract::{Form, Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Deserialize;

use crate::auth::PlayerSession;
use crate::cards;
use crate::db;
use crate::error::GameError;
use crate::hub::RoomMessage;
use crate::models::{DrawCount, Game, HouseRule, Player, Room};
use crate::render;
use crate::rules;
use crate::GameState;

/// Render the room's current game panel: active game state, or the idle
/// start panel when no game is running. `announcement` is transient — it
/// only appears in broadcast panels, never in page-load renders.
pub async fn current_panel(
    state: &GameState,
    room_id: i64,
    code: &str,
    announcement: Option<String>,
) -> String {
    match db::get_active_game(&state.pool, room_id).await {
        Some(game) if game.kind == "three_man" => {
            let (st, names) = tm_view_data(state, &game).await;
            let view = render::TmView {
                base_path: &state.base_path,
                code,
                st: &st,
                names: &names,
            };
            render::tm_phone_panel(&view)
        }
        Some(game) => active_panel(state, &game, code, announcement).await,
        None => idle_panel(state, code).await,
    }
}

pub(crate) async fn idle_panel(state: &GameState, code: &str) -> String {
    let presets = db::list_presets(&state.pool).await;
    render::game_idle_panel(&state.base_path, code, &presets)
}

/// Parses a 3 Man game's state and loads room member names — the two things
/// every `TmView` needs — from an already-fetched `Game`. Shared by
/// `current_panel`/`current_screen_panel`; `current_room_panel` reuses its
/// own already-fetched member list instead of calling this (avoids a
/// duplicate `room_members` query).
async fn tm_view_data(
    state: &GameState,
    game: &Game,
) -> (
    crate::three_man::ThreeManState,
    std::collections::HashMap<i64, String>,
) {
    let st =
        crate::three_man::ThreeManState::from_json(game.state_json.as_deref().unwrap_or_default());
    let names = db::room_members(&state.pool, game.room_id)
        .await
        .into_iter()
        .map(|m| (m.id, m.name))
        .collect();
    (st, names)
}

/// Everything needed to render either the phone's active panel or the
/// screen's active panel — computed once so both surfaces stay in sync from
/// the same DB reads within a single broadcast.
struct ActiveGameData {
    current: Option<render::CurrentCard>,
    held: Vec<render::HeldCardView>,
    remaining: i64,
    anim_key: String,
    counts: Vec<DrawCount>,
    house_rules: Vec<HouseRule>,
    kings: i64,
}

async fn load_active_game_data(state: &GameState, game: &Game) -> ActiveGameData {
    let deck = cards::parse_deck(&game.deck_order);
    let rules = rules::parse_rules(&game.rules_json);
    let draws = db::get_draws(&state.pool, game.id).await;
    let counts = db::draw_counts(&state.pool, game.id).await;
    let house_rules = db::house_rules(&state.pool, game.id).await;
    let kings = db::king_count(&state.pool, game.id).await;

    let spends = draws.iter().filter(|d| d.spent_at.is_some()).count();
    let anim_key = format!("{}-{}", draws.len(), spends);

    let current = draws.last().map(|d| {
        let card = deck[d.card_index as usize];
        let rule = rules::rule_for_rank(&rules, card.rank);
        // Jack (rank 11) with no house_rules row for this draw yet: the
        // drawer still owes the room a rule.
        let pending_rule = card.rank == 11 && !house_rules.iter().any(|hr| hr.draw_id == d.id);
        render::CurrentCard {
            card,
            title: rule.title.clone(),
            text: rule.text.clone(),
            drawer: d.player_name.clone(),
            drawer_id: d.player_id,
            draw_id: d.id,
            pending_rule,
        }
    });
    let held = draws
        .iter()
        .filter(|d| d.spent_at.is_none())
        .filter_map(|d| {
            let card = deck[d.card_index as usize];
            let rule = rules::rule_for_rank(&rules, card.rank);
            rule.holdable.then(|| render::HeldCardView {
                draw_id: d.id,
                holder_id: d.player_id,
                holder_name: d.player_name.clone(),
                card,
                title: rule.title.clone(),
            })
        })
        .collect();

    ActiveGameData {
        current,
        held,
        remaining: 52 - draws.len() as i64,
        anim_key,
        counts,
        house_rules,
        kings,
    }
}

async fn active_panel(
    state: &GameState,
    game: &Game,
    code: &str,
    announcement: Option<String>,
) -> String {
    let data = load_active_game_data(state, game).await;
    let view = render::GameView {
        base_path: &state.base_path,
        code,
        current: data.current,
        remaining: data.remaining,
        held: data.held,
        counts: &data.counts,
        announcement,
        anim_key: data.anim_key,
    };
    render::game_active_panel(&view)
}

async fn active_screen_panel(state: &GameState, game: &Game, code: &str) -> String {
    let data = load_active_game_data(state, game).await;
    let view = render::GameView {
        base_path: &state.base_path,
        code,
        current: data.current,
        remaining: data.remaining,
        held: data.held,
        counts: &data.counts,
        announcement: None,
        anim_key: data.anim_key,
    };
    render::screen_panel_active(&view, &data.house_rules, data.kings)
}

/// Render the room's current big-screen panel: active game state, or the
/// idle "no game running" panel when nothing is active.
pub async fn current_screen_panel(state: &GameState, room_id: i64, code: &str) -> String {
    match db::get_active_game(&state.pool, room_id).await {
        Some(game) if game.kind == "three_man" => {
            let (st, names) = tm_view_data(state, &game).await;
            let view = render::TmView {
                base_path: &state.base_path,
                code,
                st: &st,
                names: &names,
            };
            render::tm_screen_panel(&view)
        }
        Some(game) => active_screen_panel(state, &game, code).await,
        None => render::screen_panel_idle(code),
    }
}

/// Room-wide ROOM/TABLE tab panel: members, house rules, King's Cup fill,
/// and a `mode` derived from whatever game (if any) is active. Task 5 wires
/// this into broadcasts; Task 7 wires it into the room shell.
pub async fn current_room_panel(state: &GameState, room_id: i64, code: &str) -> String {
    let members = db::room_members(&state.pool, room_id).await;
    let (house_rules, kings, mode, seating) = match db::get_active_game(&state.pool, room_id).await
    {
        Some(game) if game.kind == "three_man" => {
            let st = crate::three_man::ThreeManState::from_json(
                game.state_json.as_deref().unwrap_or_default(),
            );
            let names: std::collections::HashMap<i64, String> =
                members.iter().map(|m| (m.id, m.name.clone())).collect();
            let view = render::TmView {
                base_path: &state.base_path,
                code,
                st: &st,
                names: &names,
            };
            (
                Vec::new(),
                0,
                game.kind,
                Some(render::tm_seating_html(&view)),
            )
        }
        Some(game) => (
            db::house_rules(&state.pool, game.id).await,
            db::king_count(&state.pool, game.id).await,
            game.kind,
            None,
        ),
        None => (Vec::new(), 0, "idle".to_string(), None),
    };
    let view = render::RoomView {
        base_path: &state.base_path,
        code,
        members: &members,
        house_rules: &house_rules,
        kings,
        mode: &mode,
        seating,
    };
    render::room_panel(&view)
}

/// Assembles post-game superlatives from three independent sources: draw
/// counts (hardest hit / most draws), the room leaderboard (most shots /
/// room total), and the King's Cup drawer. `room_total` is drinks+shots
/// logged room-wide this session — not cards drawn, which `counts` already
/// covers per-player.
async fn game_summary(state: &GameState, room_id: i64, game_id: i64) -> render::GameSummary {
    let counts = db::draw_counts(&state.pool, game_id).await;
    let hardest = counts.first().map(|c| (c.name.clone(), c.draws));
    let leaderboard = db::leaderboard(&state.pool, room_id).await;
    let most_shots = leaderboard
        .iter()
        .filter(|r| r.shots > 0)
        .max_by_key(|r| r.shots)
        .map(|r| (r.name.clone(), r.shots));
    let room_total: i64 = leaderboard.iter().map(|r| r.drinks + r.shots).sum();
    let kings_cup = db::last_king_drawer(&state.pool, game_id).await;
    let house_rules = db::house_rules(&state.pool, game_id).await;
    render::GameSummary {
        hardest,
        most_shots,
        room_total,
        kings_cup,
        counts,
        house_rules,
    }
}

/// Renders and publishes both dual-surface panels — the phone GAME tab and
/// the big-screen panel — from the same broadcast so they never drift.
pub(crate) async fn broadcast_game(
    state: &GameState,
    room_id: i64,
    code: &str,
    announcement: Option<String>,
) {
    let phone = current_panel(state, room_id, code, announcement).await;
    let screen = current_screen_panel(state, room_id, code).await;
    state.hub.publish(room_id, RoomMessage::Game(phone));
    state.hub.publish(room_id, RoomMessage::Screen(screen));
}

/// Renders and publishes the ROOM/TABLE panel — members, house rules,
/// King's Cup fill, and the idle/active mode flag.
pub(crate) async fn broadcast_room(state: &GameState, room_id: i64, code: &str) {
    let html = current_room_panel(state, room_id, code).await;
    state.hub.publish(room_id, RoomMessage::Room(html));
}

/// End-of-game broadcast: summary on top, idle panel (Start button) below,
/// on both surfaces, then a room refresh (King's Cup fill resets next game).
async fn broadcast_game_over(state: &GameState, room_id: i64, code: &str, game_id: i64) {
    let summary = game_summary(state, room_id, game_id).await;
    let phone_html = format!(
        "{}{}",
        render::game_over_panel(&summary),
        idle_panel(state, code).await
    );
    let screen_html = render::screen_panel_over(&summary);
    state.hub.publish(room_id, RoomMessage::Game(phone_html));
    state.hub.publish(room_id, RoomMessage::Screen(screen_html));
    broadcast_room(state, room_id, code).await;
}

/// Shared guard: open room + membership, mirroring log_event's checks.
/// `pub(crate)` so `tm_routes.rs`'s handlers reuse the exact same check.
pub(crate) async fn member_room(
    state: &GameState,
    code: &str,
    player: &Player,
) -> Result<Room, axum::response::Response> {
    let Some(room) = db::get_open_room(&state.pool, &code.to_uppercase()).await else {
        return Err(GameError::RoomNotFound.into_response());
    };
    if !db::is_room_member(&state.pool, room.id, player.id).await {
        return Err(StatusCode::FORBIDDEN.into_response());
    }
    Ok(room)
}

#[derive(Deserialize)]
pub struct StartForm {
    pub preset_id: i64,
}

pub async fn start_game_handler(
    State(state): State<GameState>,
    PlayerSession(player): PlayerSession,
    Path(code): Path<String>,
    Form(form): Form<StartForm>,
) -> axum::response::Response {
    let room = match member_room(&state, &code, &player).await {
        Ok(r) => r,
        Err(resp) => return resp,
    };
    let Some(preset) = db::get_preset(&state.pool, form.preset_id).await else {
        return GameError::PresetNotFound.into_response();
    };
    let deck = cards::deck_to_string(&cards::shuffled_deck());
    if let Err(e) = db::start_game(
        &state.pool,
        room.id,
        "ring_of_fire",
        &preset.rules_json,
        &deck,
        None,
    )
    .await
    {
        return e.into_response();
    }
    db::touch_room(&state.pool, room.id).await;
    broadcast_game(&state, room.id, &room.code, None).await;
    // data-mode flips idle -> ring_of_fire; without this, connected clients
    // keep the idle top bar until the next unrelated room event.
    broadcast_room(&state, room.id, &room.code).await;
    StatusCode::NO_CONTENT.into_response()
}

pub async fn draw_handler(
    State(state): State<GameState>,
    PlayerSession(player): PlayerSession,
    Path(code): Path<String>,
) -> axum::response::Response {
    let room = match member_room(&state, &code, &player).await {
        Ok(r) => r,
        Err(resp) => return resp,
    };
    let Some(game) = db::get_active_game(&state.pool, room.id).await else {
        return GameError::NoActiveGame.into_response();
    };
    if game.kind != "ring_of_fire" {
        return GameError::WrongGameKind.into_response();
    }
    let ranks: Vec<u8> = cards::parse_deck(&game.deck_order)
        .iter()
        .map(|c| c.rank)
        .collect();
    let index = match db::insert_draw(&state.pool, game.id, player.id, &ranks).await {
        Ok(i) => i,
        Err(e) => return e.into_response(),
    };
    db::touch_room(&state.pool, room.id).await;
    let drawn_rank = ranks[index as usize];
    if index == 51 {
        // Last card: render + publish the active panel showing the 52nd card
        // BEFORE ending the game. broadcast_game/current_panel goes through
        // db::get_active_game, which filters ended_at IS NULL — calling
        // end_game first would make this "final card" frame render the idle
        // panel instead of the card that was just drawn.
        let phone_html = active_panel(&state, &game, &room.code, None).await;
        let screen_html = active_screen_panel(&state, &game, &room.code).await;
        state.hub.publish(room.id, RoomMessage::Game(phone_html));
        state.hub.publish(room.id, RoomMessage::Screen(screen_html));
        db::end_game(&state.pool, game.id).await;
        broadcast_game_over(&state, room.id, &room.code, game.id).await; // …then the summary
    } else {
        broadcast_game(&state, room.id, &room.code, None).await;
        // King fill changes: refresh the ROOM/TABLE tab too.
        if drawn_rank == 13 {
            broadcast_room(&state, room.id, &room.code).await;
        }
    }
    StatusCode::NO_CONTENT.into_response()
}

#[derive(Deserialize)]
pub struct SpendForm {
    pub draw_id: i64,
}

pub async fn spend_handler(
    State(state): State<GameState>,
    PlayerSession(player): PlayerSession,
    Path(code): Path<String>,
    Form(form): Form<SpendForm>,
) -> axum::response::Response {
    let room = match member_room(&state, &code, &player).await {
        Ok(r) => r,
        Err(resp) => return resp,
    };
    let Some(game) = db::get_active_game(&state.pool, room.id).await else {
        return GameError::NoActiveGame.into_response();
    };
    if game.kind != "ring_of_fire" {
        return GameError::WrongGameKind.into_response();
    }
    if !db::spend_draw(&state.pool, game.id, form.draw_id, player.id).await {
        return GameError::CardNotHeld.into_response();
    }
    db::touch_room(&state.pool, room.id).await;
    // Announce which rule was spent ("alice used Thumb Master!").
    let deck = cards::parse_deck(&game.deck_order);
    let rules = rules::parse_rules(&game.rules_json);
    let title = db::get_draws(&state.pool, game.id)
        .await
        .iter()
        .find(|d| d.id == form.draw_id)
        .map(|d| {
            rules::rule_for_rank(&rules, deck[d.card_index as usize].rank)
                .title
                .clone()
        })
        .unwrap_or_default();
    let announcement = format!("{} used {}!", player.name, title);
    broadcast_game(&state, room.id, &room.code, Some(announcement)).await;
    StatusCode::NO_CONTENT.into_response()
}

pub async fn end_game_handler(
    State(state): State<GameState>,
    PlayerSession(player): PlayerSession,
    Path(code): Path<String>,
) -> axum::response::Response {
    let room = match member_room(&state, &code, &player).await {
        Ok(r) => r,
        Err(resp) => return resp,
    };
    let Some(game) = db::get_active_game(&state.pool, room.id).await else {
        return GameError::NoActiveGame.into_response();
    };
    if game.kind != "ring_of_fire" {
        return GameError::WrongGameKind.into_response();
    }
    db::end_game(&state.pool, game.id).await;
    db::touch_room(&state.pool, room.id).await;
    broadcast_game_over(&state, room.id, &room.code, game.id).await;
    StatusCode::NO_CONTENT.into_response()
}

#[derive(Deserialize)]
pub struct RuleForm {
    pub text: String,
}

/// Sets the house rule for the room's most recent draw. Only valid right
/// after drawing an unruled Jack (rank 11); only the drawer may set it.
pub async fn rule_handler(
    State(state): State<GameState>,
    PlayerSession(player): PlayerSession,
    Path(code): Path<String>,
    Form(form): Form<RuleForm>,
) -> axum::response::Response {
    let room = match member_room(&state, &code, &player).await {
        Ok(r) => r,
        Err(resp) => return resp,
    };
    let Some(game) = db::get_active_game(&state.pool, room.id).await else {
        return GameError::NoActiveGame.into_response();
    };
    if game.kind != "ring_of_fire" {
        return GameError::WrongGameKind.into_response();
    }
    let draws = db::get_draws(&state.pool, game.id).await;
    let Some(last) = draws.last() else {
        return GameError::OutOfTurn.into_response();
    };
    if last.rank != 11 {
        return GameError::OutOfTurn.into_response();
    }
    if last.player_id != player.id {
        return GameError::NotYourCall.into_response();
    }
    let text = form.text.trim();
    if text.is_empty() || text.chars().count() > 200 {
        return GameError::RuleTooLong.into_response();
    }
    match db::insert_house_rule(&state.pool, game.id, last.id, player.id, text).await {
        Ok(_) => {}
        // UNIQUE(draw_id) violation: someone already set this Jack's rule.
        Err(e)
            if e.as_database_error()
                .is_some_and(|d| d.is_unique_violation()) =>
        {
            return GameError::OutOfTurn.into_response();
        }
        Err(e) => return GameError::Db(e).into_response(),
    }
    db::touch_room(&state.pool, room.id).await;
    broadcast_room(&state, room.id, &room.code).await;
    broadcast_game(
        &state,
        room.id,
        &room.code,
        Some(format!("{} made a rule", player.name)),
    )
    .await;
    StatusCode::NO_CONTENT.into_response()
}
