use askama::Template;
use axum::extract::Form;
use axum::extract::Path;
use axum::extract::RawQuery;
use axum::extract::State;
use axum::http::header;
use axum::http::HeaderMap;
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{Html, IntoResponse, Redirect};
use axum::routing::{get, post};
use axum::Router;
use futures::StreamExt;
use serde::Deserialize;
use std::convert::Infallible;
use tokio_stream::wrappers::BroadcastStream;

use crate::auth::{self, OptionalPlayer, PlayerSession};
use crate::db;
use crate::error::GameError;
use crate::hub::RoomMessage;
use crate::render;
use crate::rooms;
use crate::GameState;

#[derive(Template)]
#[template(path = "landing.html")]
struct LandingTemplate {
    base_path: String,
    logged_in: bool,
    player_name: String,
    lifetime_drinks: i64,
    lifetime_shots: i64,
    lifetime_nights: i64,
    lifetime_kings: i64,
    next: String,
}

/// Only `{base}/room/<4 letters>` is accepted as a post-login destination —
/// this is the sole gate keeping the `next` query/form param from becoming
/// an open redirect (arbitrary scheme/host, path traversal, etc.).
fn valid_next(base_path: &str, next: &str) -> bool {
    let prefix = format!("{base_path}/room/");
    match next.strip_prefix(prefix.as_str()) {
        Some(rest) => rest.len() == 4 && rest.bytes().all(|b| rooms::CODE_ALPHABET.contains(&b)),
        None => false,
    }
}

/// Pulls a validated `next` value out of a raw query string. Room codes are
/// uppercase letters only, so the value is never percent-encoded and a plain
/// `split('&')`/`strip_prefix` is enough — no decoding, no serde, and no way
/// for a malformed query string to fail the extraction.
fn next_from_query(query: Option<&str>, base_path: &str) -> String {
    let Some(query) = query else {
        return String::new();
    };
    query
        .split('&')
        .find_map(|pair| pair.strip_prefix("next="))
        .filter(|n| valid_next(base_path, n))
        .unwrap_or_default()
        .to_string()
}

/// Derives an absolute `scheme://host` origin for building shareable URLs
/// (the room QR code needs an absolute URL, not a path). `X-Forwarded-Proto`
/// (set by nginx — see deploy/nginx.conf) wins when present; otherwise guess
/// `http` for local dev hosts and `https` everywhere else.
pub fn request_origin(headers: &HeaderMap) -> String {
    let host = headers
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("localhost");
    let scheme = headers
        .get("x-forwarded-proto")
        .and_then(|h| h.to_str().ok())
        // A chain of proxies sends a comma-separated list ("https, http") —
        // only the first hop's value describes what the client actually used.
        .and_then(|v| v.split(',').next())
        .map(str::trim)
        .filter(|s| *s == "http" || *s == "https")
        .map(str::to_string)
        .unwrap_or_else(|| {
            if host.starts_with("localhost") || host.starts_with("127.") {
                "http".to_string()
            } else {
                "https".to_string()
            }
        });
    format!("{scheme}://{host}")
}

#[derive(Template)]
#[template(path = "error.html")]
pub struct ErrorTemplate {
    base_path: String,
    message: String,
}

/// Full-page friendly error (unknown/ended room, etc.) with a link home.
pub fn error_page(
    state: &GameState,
    status: axum::http::StatusCode,
    message: &str,
) -> axum::response::Response {
    let tpl = ErrorTemplate {
        base_path: state.base_path.to_string(),
        message: message.to_string(),
    };
    (status, Html(tpl.render().unwrap())).into_response()
}

#[derive(Template)]
#[template(path = "room.html")]
struct RoomTemplate {
    base_path: String,
    code: String,
    player_id: i64,
    leaderboard_items: String,
    game_panel: String,
    room_panel: String,
}

async fn create_room(
    State(state): State<GameState>,
    PlayerSession(player): PlayerSession,
) -> impl IntoResponse {
    let room = rooms::create_room_with_unique_code(&state.pool).await;
    db::join_room(&state.pool, room.id, player.id).await;
    Redirect::to(&format!("{}/room/{}", state.base_path, room.code))
}

#[derive(Deserialize)]
struct JoinForm {
    code: String,
}

async fn join_room_handler(
    State(state): State<GameState>,
    PlayerSession(_player): PlayerSession,
    Form(form): Form<JoinForm>,
) -> axum::response::Response {
    let code = form.code.trim().to_uppercase();
    match db::get_open_room(&state.pool, &code).await {
        // The room page itself performs the join — one code path for both
        // form joins and shared-link joins.
        Some(_) => Redirect::to(&format!("{}/room/{code}", state.base_path)).into_response(),
        None => error_page(&state, StatusCode::NOT_FOUND, "No open room with that code"),
    }
}

async fn room_page(
    State(state): State<GameState>,
    PlayerSession(player): PlayerSession,
    Path(code): Path<String>,
) -> axum::response::Response {
    let code = code.to_uppercase();
    let Some(room) = db::get_open_room(&state.pool, &code).await else {
        return error_page(
            &state,
            StatusCode::NOT_FOUND,
            "Room not found or already ended",
        );
    };
    // Visiting a room joins it: room URLs double as invite links.
    db::join_room(&state.pool, room.id, player.id).await;

    // Mid-game join: a running 3 Man round tracks seating in its own
    // `order` field (not just room_players), so a newcomer needs appending
    // there too or they'd never get a turn. Locked across load -> mutate ->
    // persist -> broadcast, the same discipline every `/tm/*` action
    // handler uses — releasing the lock before broadcasting would let a
    // concurrent action handler's broadcast land first, leaving this
    // request's now-stale render as the last word on every client's screen.
    {
        let lock = state.locks.for_room(room.id);
        let _guard = lock.lock().await;
        let mut tm_joined = false;
        // Hoisted out of the `if game.kind == "last_call"` arm below, and
        // doubling as the "was this a Last Call game" flag (`Some` iff the
        // arm ran) so there's one variable to keep in sync, not two: stays
        // reachable at the redirect's `return`, inside this same lock guard,
        // for Task 3's `broadcast_lc(&state, room.id, &st).await` call,
        // which needs the freshly (maybe-)seated state, not a re-read.
        let mut lc_state: Option<crate::last_call::LastCallState> = None;
        // Set alongside `lc_state` only when `add_player` actually ran below
        // — a revisit by an already-seated player mutates nothing, so it
        // must not trigger a broadcast either.
        let mut lc_joined = false;
        if let Some(game) = db::get_active_game(&state.pool, room.id).await {
            if game.kind == "three_man" {
                let mut st = crate::three_man::ThreeManState::from_json(
                    game.state_json.as_deref().unwrap_or_default(),
                );
                // Already seated (a revisit, not a fresh join): skip the
                // rewrite and the extra broadcast — an unchanged order needs
                // neither.
                if !st.order.contains(&player.id) {
                    st.add_player(player.id);
                    db::set_game_state(&state.pool, game.id, &st.to_json()).await;
                    tm_joined = true;
                }
            }
            if game.kind == "last_call" {
                // Same mid-game-join problem 3 Man has above: LastCallState
                // seats players in its own `players[]`, so a newcomer who
                // opens the room link needs seating there too or they have
                // no hand and no plaque. Same lock discipline — releasing
                // the lock before the broadcast would let a concurrent
                // handler's broadcast land first and leave this request's
                // stale render as the last word.
                let mut st = crate::last_call::LastCallState::from_json(
                    game.state_json.as_deref().unwrap_or_default(),
                );
                // `add_player` returns `None` when the table is already at
                // `MAX_SEATS` — the newcomer must still join the room (that
                // already happened above, via `db::join_room`), just not
                // this game, so a full table is not an error here.
                if st.seat_of(player.id).is_none()
                    && st.add_player(player.id, &player.name).is_some()
                {
                    db::set_game_state(&state.pool, game.id, &st.to_json()).await;
                    lc_joined = true;
                }
                lc_state = Some(st);
            }
        }
        // New members appear on everyone's ROOM tab without waiting for the
        // next unrelated event.
        crate::game::broadcast_room(&state, room.id, &code).await;
        if tm_joined {
            crate::game::broadcast_game(&state, room.id, &code, None).await;
        }
        if let Some(st) = &lc_state {
            if lc_joined {
                // No `broadcast_game` call here (unlike `tm_joined` above,
                // and unlike `persist_and_broadcast_lc`, which the plan-end
                // review's finding I1 fixed to call it): `lc_placeholder_panel`
                // and `lc_screen_placeholder` are static text plus
                // `base_path`/`code`, so a late join changes nothing they
                // render. What DOES change — the member list — is exactly
                // what `broadcast_room` above already covers. Revisit this
                // if Plan B's GAME-tab content ever starts depending on
                // seating/roster.
                crate::lc_routes::broadcast_lc(&state, room.id, st).await;
            }
            return Redirect::to(&format!("{}/room/{}/lastcall", state.base_path, code))
                .into_response();
        }
    }
    let leaderboard_items = render_leaderboard(&state, room.id).await;
    let game_panel = crate::game::current_panel(&state, room.id, &code, None).await;
    let room_panel = crate::game::current_room_panel(&state, room.id, &code).await;
    let tpl = RoomTemplate {
        base_path: state.base_path.to_string(),
        code,
        player_id: player.id,
        leaderboard_items,
        game_panel,
        room_panel,
    };
    Html(tpl.render().unwrap()).into_response()
}

async fn landing(
    State(state): State<GameState>,
    OptionalPlayer(player): OptionalPlayer,
    // RawQuery, not Query<HashMap<..>>: this is the app's front door, so a
    // malformed query string must never fail extraction — an unrecognized
    // shape just means no `next`, handled by `next_from_query` returning "".
    RawQuery(query): RawQuery,
) -> impl IntoResponse {
    let next = next_from_query(query.as_deref(), &state.base_path);
    let tpl = match player {
        Some(p) => {
            let (drinks, shots) = db::lifetime_counts(&state.pool, p.id).await;
            let nights = db::lifetime_nights(&state.pool, p.id).await;
            let kings = db::lifetime_kings(&state.pool, p.id).await;
            LandingTemplate {
                base_path: state.base_path.to_string(),
                logged_in: true,
                player_name: p.name,
                lifetime_drinks: drinks,
                lifetime_shots: shots,
                lifetime_nights: nights,
                lifetime_kings: kings,
                next,
            }
        }
        None => LandingTemplate {
            base_path: state.base_path.to_string(),
            logged_in: false,
            player_name: String::new(),
            lifetime_drinks: 0,
            lifetime_shots: 0,
            lifetime_nights: 0,
            lifetime_kings: 0,
            next,
        },
    };
    Html(tpl.render().unwrap())
}

#[derive(Deserialize)]
struct LoginForm {
    name: String,
    pin: String,
    next: Option<String>,
}

async fn login(
    State(state): State<GameState>,
    Form(form): Form<LoginForm>,
) -> axum::response::Response {
    match auth::login_or_register(&state.pool, &form.name, &form.pin).await {
        Ok(player) => {
            let sid = auth::new_session_id();
            db::create_session(&state.pool, &sid, player.id, "+90 days").await;
            let dest = match &form.next {
                Some(n) if valid_next(&state.base_path, n) => n.clone(),
                _ => format!("{}/", state.base_path),
            };
            (
                [(header::SET_COOKIE, auth::session_cookie(&sid))],
                Redirect::to(&dest),
            )
                .into_response()
        }
        // The login form is a plain (non-HTMX) post, so render the friendly
        // full error page — a bare fragment would arrive unstyled.
        Err(e @ GameError::WrongPin) => {
            error_page(&state, axum::http::StatusCode::UNAUTHORIZED, &e.to_string())
        }
        Err(e @ (GameError::InvalidName | GameError::InvalidPin)) => error_page(
            &state,
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            &e.to_string(),
        ),
        Err(e) => e.into_response(),
    }
}

#[derive(Template)]
#[template(path = "account.html")]
struct AccountTemplate {
    base_path: String,
    player_name: String,
    lifetime_drinks: i64,
    lifetime_shots: i64,
    lifetime_nights: i64,
}

async fn account_page(
    State(state): State<GameState>,
    PlayerSession(player): PlayerSession,
) -> impl IntoResponse {
    let (drinks, shots) = db::lifetime_counts(&state.pool, player.id).await;
    let nights = db::lifetime_nights(&state.pool, player.id).await;
    let tpl = AccountTemplate {
        base_path: state.base_path.to_string(),
        player_name: player.name,
        lifetime_drinks: drinks,
        lifetime_shots: shots,
        lifetime_nights: nights,
    };
    Html(tpl.render().unwrap())
}

#[derive(Deserialize)]
struct NameForm {
    name: String,
}

async fn change_name(
    State(state): State<GameState>,
    PlayerSession(player): PlayerSession,
    Form(form): Form<NameForm>,
) -> axum::response::Response {
    match auth::rename_player(&state.pool, player.id, &form.name).await {
        Ok(_) => {
            // Names are baked into already-rendered HTML on every connected
            // phone and big screen (standings, WHO'S HERE, draw log, house
            // rules), so a mid-night rename has to push all three surfaces
            // for each open room or the old name lingers until the next
            // unrelated event. No room lock: nothing here mutates game state.
            for room in db::open_rooms_for_player(&state.pool, player.id).await {
                broadcast_leaderboard(&state, room.id).await;
                crate::game::broadcast_room(&state, room.id, &room.code).await;
                // Only while a game is actually running. With none active,
                // `broadcast_game` publishes the idle panel — which carries no
                // player names to refresh, and would overwrite a game-over
                // summary still on everyone's phone and on the big screen.
                if db::get_active_game(&state.pool, room.id).await.is_some() {
                    crate::game::broadcast_game(&state, room.id, &room.code, None).await;
                }
            }
            Redirect::to(&format!("{}/account", state.base_path)).into_response()
        }
        Err(e @ GameError::NameTaken) => error_page(&state, StatusCode::CONFLICT, &e.to_string()),
        Err(e @ GameError::InvalidName) => {
            error_page(&state, StatusCode::UNPROCESSABLE_ENTITY, &e.to_string())
        }
        Err(e) => e.into_response(),
    }
}

#[derive(Deserialize)]
struct PinForm {
    current_pin: String,
    new_pin: String,
}

async fn change_pin(
    State(state): State<GameState>,
    PlayerSession(player): PlayerSession,
    Form(form): Form<PinForm>,
) -> axum::response::Response {
    match auth::change_pin(&state.pool, player, &form.current_pin, &form.new_pin).await {
        Ok(()) => Redirect::to(&format!("{}/account", state.base_path)).into_response(),
        Err(GameError::WrongPin) => error_page(
            &state,
            StatusCode::UNAUTHORIZED,
            "That's not your current PIN",
        ),
        Err(e @ GameError::InvalidPin) => {
            error_page(&state, StatusCode::UNPROCESSABLE_ENTITY, &e.to_string())
        }
        Err(e) => e.into_response(),
    }
}

/// Signs this phone out — the player's other devices keep their sessions.
/// Deletes the row as well as clearing the cookie: a cookie-only logout
/// would leave a 90-day-valid session id behind.
async fn logout(State(state): State<GameState>, headers: HeaderMap) -> axum::response::Response {
    if let Some(sid) = auth::extract_session_cookie(&headers) {
        db::delete_session(&state.pool, &sid).await;
    }
    (
        [(header::SET_COOKIE, auth::clear_session_cookie())],
        Redirect::to(&format!("{}/", state.base_path)),
    )
        .into_response()
}

/// Renders the standings, tagging the current 3 Man's row with the badge
/// when a 3 Man game is active. Shared by `broadcast_leaderboard` and the
/// SSE reconnect snapshot so a fresh connection carries the badge too.
async fn render_leaderboard(state: &GameState, room_id: i64) -> String {
    let rows = db::leaderboard(&state.pool, room_id).await;
    let three_man = match db::get_active_game(&state.pool, room_id).await {
        Some(game) if game.kind == "three_man" => {
            let st = crate::three_man::ThreeManState::from_json(
                game.state_json.as_deref().unwrap_or_default(),
            );
            Some(st.three_man)
        }
        _ => None,
    };
    render::leaderboard_items_tm(&rows, three_man)
}

/// Re-render the standings and push to every subscribed screen in the room.
pub(crate) async fn broadcast_leaderboard(state: &GameState, room_id: i64) {
    let html = render_leaderboard(state, room_id).await;
    state
        .hub
        .publish(room_id, crate::hub::RoomMessage::Leaderboard(html));
}

#[derive(Deserialize)]
struct EventForm {
    kind: String,
}

async fn log_event(
    State(state): State<GameState>,
    PlayerSession(player): PlayerSession,
    Path(code): Path<String>,
    Form(form): Form<EventForm>,
) -> axum::response::Response {
    if form.kind != "drink" && form.kind != "shot" {
        return StatusCode::UNPROCESSABLE_ENTITY.into_response();
    }
    let Some(room) = db::get_open_room(&state.pool, &code.to_uppercase()).await else {
        return GameError::RoomNotFound.into_response();
    };
    if !db::is_room_member(&state.pool, room.id, player.id).await {
        return StatusCode::FORBIDDEN.into_response();
    }
    db::insert_event(&state.pool, room.id, player.id, &form.kind).await;
    db::touch_room(&state.pool, room.id).await;
    crate::mechanics::on_event(room.id, player.id, &form.kind);
    broadcast_leaderboard(&state, room.id).await;
    // Auto-logged verdict drinks (mechanics) never reach here — only a
    // player's own drink/shot tap does, so this always fires an emote.
    let glyph = if form.kind == "drink" { "🍺" } else { "🥃" };
    state.hub.publish(room.id, RoomMessage::Emote(glyph.into()));
    StatusCode::NO_CONTENT.into_response()
}

async fn undo_event(
    State(state): State<GameState>,
    PlayerSession(player): PlayerSession,
    Path(code): Path<String>,
) -> axum::response::Response {
    let Some(room) = db::get_open_room(&state.pool, &code.to_uppercase()).await else {
        return GameError::RoomNotFound.into_response();
    };
    if !db::is_room_member(&state.pool, room.id, player.id).await {
        return StatusCode::FORBIDDEN.into_response();
    }
    if db::undo_last_event(&state.pool, room.id, player.id).await {
        db::touch_room(&state.pool, room.id).await;
        broadcast_leaderboard(&state, room.id).await;
    }
    StatusCode::NO_CONTENT.into_response()
}

async fn end_room_handler(
    State(state): State<GameState>,
    PlayerSession(player): PlayerSession,
    Path(code): Path<String>,
) -> axum::response::Response {
    let Some(room) = db::get_open_room(&state.pool, &code.to_uppercase()).await else {
        return GameError::RoomNotFound.into_response();
    };
    if !db::is_room_member(&state.pool, room.id, player.id).await {
        return StatusCode::FORBIDDEN.into_response();
    }
    db::end_room(&state.pool, room.id).await;
    state.hub.publish(room.id, crate::hub::RoomMessage::Ended);
    state.hub.remove(room.id);
    state.locks.remove(room.id);
    Redirect::to(&format!("{}/", state.base_path)).into_response()
}

async fn sse_stream(
    State(state): State<GameState>,
    Path(code): Path<String>,
) -> axum::response::Response {
    let Some(room) = db::get_open_room(&state.pool, &code.to_uppercase()).await else {
        return GameError::RoomNotFound.into_response();
    };

    // Subscribe BEFORE rendering the snapshot — no update can slip between.
    let rx = state.hub.subscribe(room.id);
    // Re-check after subscribing: if the room was ended in the window between
    // the lookup and the subscribe, the subscribe above resurrected a channel
    // entry that the end path already removed — drop it again and tell the
    // client the night is over instead of leaking a zombie hub entry.
    if db::get_open_room(&state.pool, &room.code).await.is_none() {
        state.hub.remove(room.id);
        state.locks.remove(room.id);
        let stream = futures::stream::once(async move {
            // A named SSE event with an empty data buffer is silently
            // dropped by the browser's EventSource parser (WHATWG SSE spec:
            // no event fires if the data buffer is empty at dispatch time)
            // — the payload's content doesn't matter, only that a `data:`
            // field is present at all.
            Ok::<_, Infallible>(Event::default().event("ended").data("gone"))
        });
        return (
            [(header::HeaderName::from_static("x-accel-buffering"), "no")],
            Sse::new(stream).keep_alive(KeepAlive::default()),
        )
            .into_response();
    }
    let initial = render_leaderboard(&state, room.id).await;
    let initial_game = crate::game::current_panel(&state, room.id, &room.code, None).await;
    let initial_screen = crate::game::current_screen_panel(&state, room.id, &room.code).await;
    let initial_room = crate::game::current_room_panel(&state, room.id, &room.code).await;
    // A fifth snapshot frame, sent only for a Last Call room — Ring of Fire
    // and 3 Man rooms must still emit exactly the four frames above.
    let lc_initial = match db::get_active_game(&state.pool, room.id).await {
        Some(game) if game.kind == "last_call" => {
            let st = crate::last_call::LastCallState::from_json(
                game.state_json.as_deref().unwrap_or_default(),
            );
            Some(crate::lc_render::lc_public_panel(&st.public_view()))
        }
        _ => None,
    };

    let stream = futures::stream::iter([
        Ok::<_, Infallible>(Event::default().event("leaderboard").data(initial)),
        Ok::<_, Infallible>(Event::default().event("game").data(initial_game)),
        Ok::<_, Infallible>(Event::default().event("screen").data(initial_screen)),
        Ok::<_, Infallible>(Event::default().event("room").data(initial_room)),
    ])
    .chain(futures::stream::iter(lc_initial.into_iter().map(|html| {
        Ok::<_, Infallible>(Event::default().event("lcpublic").data(html))
    })))
    .chain(BroadcastStream::new(rx).filter_map(|msg| async move {
        match msg {
            Ok(RoomMessage::Leaderboard(html)) => {
                Some(Ok(Event::default().event("leaderboard").data(html)))
            }
            Ok(RoomMessage::Game(html)) => Some(Ok(Event::default().event("game").data(html))),
            Ok(RoomMessage::Screen(html)) => Some(Ok(Event::default().event("screen").data(html))),
            Ok(RoomMessage::Room(html)) => Some(Ok(Event::default().event("room").data(html))),
            Ok(RoomMessage::Emote(glyph)) => Some(Ok(Event::default().event("emote").data(glyph))),
            Ok(RoomMessage::LcPublic(html)) => {
                Some(Ok(Event::default().event("lcpublic").data(html)))
            }
            // seq is a u64, so `seq.to_string()` is never empty — unlike
            // Emote/LcPublic above, there's no user-controllable value here
            // that could shrink to "", so this arm never hits the
            // empty-data-buffer pitfall the two `Ended` branches carry.
            Ok(RoomMessage::LcTick(seq)) => {
                Some(Ok(Event::default().event("lctick").data(seq.to_string())))
            }
            // Same empty-data-buffer pitfall as the zombie-reconnect branch
            // above: a `data:` field must be present or EventSource drops
            // the event silently and clients never learn the room ended.
            Ok(RoomMessage::Ended) => Some(Ok(Event::default().event("ended").data("gone"))),
            // Lagged receiver: skip. True for every *other* variant here —
            // each carries a freshly rendered fragment, so the next one that
            // arrives is a complete replacement. NOT true for `LcTick`: it
            // carries no state, only a seq number, so a lagged receiver that
            // drops a tick simply never re-fetches its hand for that change
            // and can stay stale until some unrelated later change ticks
            // again (plan-end review finding M5; harmless today at 128
            // buffered messages and three publishes per change — ~43
            // unacknowledged changes to trigger — but worth knowing).
            Err(_) => None,
        }
    }));

    (
        // Belt-and-braces alongside nginx's proxy_buffering off.
        [(header::HeaderName::from_static("x-accel-buffering"), "no")],
        Sse::new(stream).keep_alive(KeepAlive::default()),
    )
        .into_response()
}

#[derive(Template)]
#[template(path = "screen.html")]
struct ScreenTemplate {
    base_path: String,
    code: String,
    leaderboard_items: String,
    game_panel: String,
    qr_svg: String,
}

/// The Last Call big-screen shell (Task 4). Public, like `ScreenTemplate` —
/// a TV in the corner has no cookie — so `screen_page` builds this straight
/// from `db::get_active_game` + `LastCallState::from_json`, never through
/// `lc_routes::load_lc`, which requires a `PlayerSession`.
#[derive(Template)]
#[template(path = "lc_screen.html")]
struct LcScreenTemplate {
    base_path: String,
    code: String,
    round: u32,
    /// Seeds the page's `lcSeq`, exactly as `lc_room.html` seeds from
    /// `#lc-hand`'s `data-seq` — not in the brief's literal field list, but
    /// without it the page has no seq floor to stale-drop against on its
    /// first `lcpublic` frame.
    seq: u64,
    banner: String, // lc_render::lc_banner(&view)
    panel: String,  // lc_render::lc_screen_panel(&view)
}

async fn screen_page(
    State(state): State<GameState>,
    Path(code): Path<String>,
    headers: HeaderMap,
) -> axum::response::Response {
    let code = code.to_uppercase();
    let Some(room) = db::get_open_room(&state.pool, &code).await else {
        return error_page(
            &state,
            StatusCode::NOT_FOUND,
            "Room not found or already ended",
        );
    };
    // Last Call gets its own template — the generic screen.html has nowhere
    // to put a felt. Ring of Fire and 3 Man fall through to the existing
    // path below completely unchanged.
    if let Some(game) = db::get_active_game(&state.pool, room.id).await {
        if game.kind == "last_call" {
            let st = crate::last_call::LastCallState::from_json(
                game.state_json.as_deref().unwrap_or_default(),
            );
            let view = st.public_view();
            let tpl = LcScreenTemplate {
                base_path: state.base_path.to_string(),
                code,
                round: view.round,
                seq: view.seq,
                banner: crate::lc_render::lc_banner(&view),
                panel: crate::lc_render::lc_screen_panel(&view),
            };
            return Html(tpl.render().unwrap()).into_response();
        }
    }
    let leaderboard_items = render_leaderboard(&state, room.id).await;
    // The screen-scale panel builder, not the phone one — the phone idle
    // panel carries a navigable "Edit rule presets" link that has no
    // business being reachable from an unauthenticated spectator surface.
    let game_panel = crate::game::current_screen_panel(&state, room.id, &code).await;
    let join_url = format!(
        "{}{}/room/{code}",
        request_origin(&headers),
        state.base_path
    );
    let tpl = ScreenTemplate {
        base_path: state.base_path.to_string(),
        code,
        leaderboard_items,
        game_panel,
        qr_svg: render::qr_svg(&join_url),
    };
    Html(tpl.render().unwrap()).into_response()
}

async fn game_css() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css")],
        include_str!("../assets/game.css"),
    )
}

async fn lastcall_css() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css")],
        include_str!("../assets/lastcall.css"),
    )
}

/// Single htmx copy for the whole repo: embed the portfolio's vendored file.
async fn htmx_js() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "application/javascript")],
        include_str!("../../static/htmx.min.js"),
    )
}

async fn lc_motion_js() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "application/javascript")],
        include_str!("../assets/lc_motion.js"),
    )
}

/// Self-hosted webfonts — no third-party font requests from served pages.
/// Embedded via include_bytes! so the binary is self-contained; unknown
/// names (including any path-traversal attempt that reaches the handler)
/// 404 rather than touching the filesystem.
async fn font_asset(Path(name): Path<String>) -> axum::response::Response {
    let bytes: &'static [u8] = match name.as_str() {
        "archivo-500.woff2" => include_bytes!("../assets/fonts/archivo-500.woff2"),
        "archivo-600.woff2" => include_bytes!("../assets/fonts/archivo-600.woff2"),
        "archivo-700.woff2" => include_bytes!("../assets/fonts/archivo-700.woff2"),
        "archivo-800.woff2" => include_bytes!("../assets/fonts/archivo-800.woff2"),
        "archivo-900.woff2" => include_bytes!("../assets/fonts/archivo-900.woff2"),
        "space-grotesk-400.woff2" => include_bytes!("../assets/fonts/space-grotesk-400.woff2"),
        "space-grotesk-500.woff2" => include_bytes!("../assets/fonts/space-grotesk-500.woff2"),
        "space-grotesk-600.woff2" => include_bytes!("../assets/fonts/space-grotesk-600.woff2"),
        "space-grotesk-700.woff2" => include_bytes!("../assets/fonts/space-grotesk-700.woff2"),
        _ => return StatusCode::NOT_FOUND.into_response(),
    };
    (
        [
            (header::CONTENT_TYPE, "font/woff2"),
            (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
        ],
        bytes,
    )
        .into_response()
}

/// Sound-effect drop-in: files are never committed (no mp3s in the repo),
/// so the game ships silent until an admin drops allowlisted mp3s into
/// DRINKS_SOUNDS_DIR. Non-allowlisted names 404 without touching disk.
const SOUND_FILES: [&str; 6] = [
    "drink.mp3",
    "shot.mp3",
    "card-draw.mp3",
    "card-use.mp3",
    "dice-roll.mp3",
    "dice-give.mp3",
];

async fn sound_asset(Path(name): Path<String>) -> axum::response::Response {
    if !SOUND_FILES.contains(&name.as_str()) {
        return StatusCode::NOT_FOUND.into_response();
    }
    let dir = std::env::var("DRINKS_SOUNDS_DIR").unwrap_or_else(|_| "drinks-sounds".into());
    match tokio::fs::read(std::path::Path::new(&dir).join(&name)).await {
        Ok(bytes) => ([(header::CONTENT_TYPE, "audio/mpeg")], bytes).into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

pub fn router() -> Router<GameState> {
    Router::new()
        .route("/", get(landing))
        .route("/login", post(login))
        .route("/logout", post(logout))
        .route("/account", get(account_page))
        .route("/account/name", post(change_name))
        .route("/account/pin", post(change_pin))
        .route("/rooms", post(create_room))
        .route("/join", post(join_room_handler))
        .route("/room/{code}", get(room_page))
        .route("/room/{code}/event", post(log_event))
        .route("/room/{code}/undo", post(undo_event))
        .route("/room/{code}/end", post(end_room_handler))
        .route(
            "/room/{code}/game/start",
            post(crate::game::start_game_handler),
        )
        .route("/room/{code}/game/draw", post(crate::game::draw_handler))
        .route("/room/{code}/game/spend", post(crate::game::spend_handler))
        .route("/room/{code}/game/end", post(crate::game::end_game_handler))
        .route("/room/{code}/game/rule", post(crate::game::rule_handler))
        .route(
            "/room/{code}/tm/start",
            post(crate::tm_routes::tm_start_handler),
        )
        .route(
            "/room/{code}/tm/end",
            post(crate::tm_routes::tm_end_handler),
        )
        .route(
            "/room/{code}/tm/roll",
            post(crate::tm_routes::tm_roll_handler),
        )
        .route(
            "/room/{code}/tm/three-man",
            post(crate::tm_routes::tm_three_man_handler),
        )
        .route(
            "/room/{code}/tm/mode",
            post(crate::tm_routes::tm_mode_handler),
        )
        .route(
            "/room/{code}/tm/target",
            post(crate::tm_routes::tm_target_handler),
        )
        .route(
            "/room/{code}/tm/clear-slot",
            post(crate::tm_routes::tm_clear_slot_handler),
        )
        .route(
            "/room/{code}/tm/send",
            post(crate::tm_routes::tm_send_handler),
        )
        .route(
            "/room/{code}/tm/gift-roll",
            post(crate::tm_routes::tm_gift_roll_handler),
        )
        .route(
            "/room/{code}/tm/pass",
            post(crate::tm_routes::tm_pass_handler),
        )
        .route(
            "/room/{code}/tm/seat",
            post(crate::tm_routes::tm_seat_handler),
        )
        .route(
            "/room/{code}/lastcall/start",
            post(crate::lc_routes::lc_start_handler),
        )
        .route(
            "/room/{code}/lastcall/vessel",
            post(crate::lc_routes::lc_vessel_handler),
        )
        .route(
            "/room/{code}/lastcall/handicap",
            post(crate::lc_routes::lc_handicap_handler),
        )
        .route("/room/{code}/lastcall", get(crate::lc_routes::lc_page))
        .route(
            "/room/{code}/lastcall/hand",
            get(crate::lc_routes::lc_hand_handler),
        )
        .route(
            "/room/{code}/lastcall/table",
            get(crate::lc_routes::lc_table_handler),
        )
        .route(
            "/room/{code}/lastcall/end",
            post(crate::lc_routes::lc_end_handler),
        )
        .route("/room/{code}/sse", get(sse_stream))
        .route("/room/{code}/screen", get(screen_page))
        .route("/assets/game.css", get(game_css))
        .route("/assets/lastcall.css", get(lastcall_css))
        .route("/assets/htmx.min.js", get(htmx_js))
        .route("/assets/lc_motion.js", get(lc_motion_js))
        .route("/assets/fonts/{name}", get(font_asset))
        .route("/assets/sounds/{name}", get(sound_asset))
        .route("/lastcall/preview", get(crate::lc_preview::preview_page))
        .route(
            "/presets",
            get(crate::presets::presets_page).post(crate::presets::create_preset),
        )
        .route(
            "/presets/{id}",
            get(crate::presets::edit_preset_page).post(crate::presets::save_preset),
        )
        .route(
            "/presets/{id}/delete",
            post(crate::presets::delete_preset_handler),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_next() {
        assert!(valid_next("/drinks", "/drinks/room/QKAM"));
        assert!(!valid_next("/drinks", "/drinks/room/qkam"));
        assert!(!valid_next("/drinks", "https://evil.example/x"));
        assert!(!valid_next("/drinks", "/drinks/room/QKAM/../admin"));
        assert!(!valid_next("", "/room/QKAM/extra"));
    }

    #[test]
    fn test_next_from_query() {
        assert_eq!(
            next_from_query(Some("next=/drinks/room/QKAM"), "/drinks"),
            "/drinks/room/QKAM"
        );
        assert_eq!(next_from_query(None, "/drinks"), "");
        assert_eq!(
            next_from_query(Some("next=https://evil.example/x"), "/drinks"),
            ""
        );
        assert_eq!(
            next_from_query(Some("foo=bar&next=/drinks/room/QKAM"), "/drinks"),
            "/drinks/room/QKAM"
        );
    }

    #[test]
    fn test_request_origin() {
        let headers = |pairs: &[(&str, &str)]| {
            let mut h = HeaderMap::new();
            for (k, v) in pairs {
                h.insert(
                    axum::http::HeaderName::from_bytes(k.as_bytes()).unwrap(),
                    v.parse().unwrap(),
                );
            }
            h
        };
        assert_eq!(
            request_origin(&headers(&[("host", "example.com")])),
            "https://example.com"
        );
        assert_eq!(
            request_origin(&headers(&[("host", "localhost:3001")])),
            "http://localhost:3001"
        );
        assert_eq!(
            request_origin(&headers(&[
                ("x-forwarded-proto", "https"),
                ("host", "localhost"),
            ])),
            "https://localhost"
        );
    }
}
