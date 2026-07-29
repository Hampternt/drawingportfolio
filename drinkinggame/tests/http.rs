//! Handler-level integration tests via tower::ServiceExt, portfolio-style.

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use sqlx::sqlite::SqlitePoolOptions;
use tower::ServiceExt;

async fn test_app_with_pool() -> (Router, sqlx::SqlitePool) {
    // max_connections(1): a :memory: db exists per-connection.
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    drinkinggame::db::run_migrations(&pool).await;
    (drinkinggame::router_with_pool(pool.clone(), ""), pool)
}

async fn test_app() -> Router {
    test_app_with_pool().await.0
}

async fn body_string(res: axum::response::Response) -> String {
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    String::from_utf8(bytes.to_vec()).unwrap()
}

#[tokio::test]
async fn test_landing_serves_login_form() {
    let app = test_app().await;
    let res = app
        .oneshot(Request::get("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let html = body_string(res).await;
    assert!(html.contains(r#"action="/login""#));
    assert!(html.contains("4-digit PIN"));
}

#[tokio::test]
async fn test_assets_are_served() {
    let app = test_app().await;
    for (path, ct) in [
        ("/assets/game.css", "text/css"),
        ("/assets/htmx.min.js", "application/javascript"),
    ] {
        let res = app
            .clone()
            .oneshot(Request::get(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK, "{path}");
        assert_eq!(res.headers()[header::CONTENT_TYPE], ct, "{path}");
    }
}

/// Logs in (registering on first use) and returns the "dg_session=..." pair.
async fn login(app: &Router, name: &str, pin: &str) -> String {
    let res = app
        .clone()
        .oneshot(
            Request::post("/login")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(format!("name={name}&pin={pin}")))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    res.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string()
}

#[tokio::test]
async fn test_login_sets_cookie_and_landing_recognizes_it() {
    let app = test_app().await;
    let cookie = login(&app, "hampter", "1234").await;
    assert!(cookie.starts_with("dg_session="));

    let res = app
        .oneshot(
            Request::get("/")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let html = body_string(res).await;
    assert!(html.contains("hampter"));
    assert!(html.contains("Lifetime: 0 drinks"));
}

#[tokio::test]
async fn test_wrong_pin_is_rejected() {
    let app = test_app().await;
    login(&app, "hampter", "1234").await;
    let res = app
        .oneshot(
            Request::post("/login")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from("name=hampter&pin=9999"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    assert!(body_string(res).await.contains("wrong PIN"));
}

/// Creates a room as `cookie`'s player and returns the room code.
async fn create_room(app: &Router, cookie: &str) -> String {
    let res = app
        .clone()
        .oneshot(
            Request::post("/rooms")
                .header(header::COOKIE, cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let loc = res.headers()[header::LOCATION].to_str().unwrap();
    loc.rsplit('/').next().unwrap().to_string()
}

#[tokio::test]
async fn test_create_room_and_view_it() {
    let app = test_app().await;
    let cookie = login(&app, "alice", "1234").await;
    let code = create_room(&app, &cookie).await;
    assert_eq!(code.len(), 4);

    let res = app
        .oneshot(
            Request::get(format!("/room/{code}"))
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let html = body_string(res).await;
    assert!(html.contains("+1 Drink"));
    assert!(html.contains("alice")); // creator is on the leaderboard
}

#[tokio::test]
async fn test_visiting_room_auto_joins() {
    let app = test_app().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let code = create_room(&app, &alice).await;

    // Bob opens the shared link — no explicit join step.
    let res = app
        .clone()
        .oneshot(
            Request::get(format!("/room/{code}"))
                .header(header::COOKIE, &bob)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let html = body_string(res).await;
    assert!(html.contains("alice") && html.contains("bob"));
}

#[tokio::test]
async fn test_room_requires_session_and_dead_codes_are_friendly() {
    let app = test_app().await;
    // No cookie -> redirected to landing.
    let res = app
        .clone()
        .oneshot(Request::get("/room/XXXX").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    // Cookie but nonexistent room -> friendly 404 page with a home link.
    let cookie = login(&app, "alice", "1234").await;
    let res = app
        .oneshot(
            Request::get("/room/XXXX")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert!(body_string(res).await.contains("Back home"));
}

async fn post_form(app: &Router, cookie: &str, path: &str, body: &str) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::post(path)
                .header(header::COOKIE, cookie)
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn room_page_html(app: &Router, cookie: &str, code: &str) -> String {
    let res = app
        .clone()
        .oneshot(
            Request::get(format!("/room/{code}"))
                .header(header::COOKIE, cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    body_string(res).await
}

#[tokio::test]
async fn test_log_undo_and_leaderboard_counts() {
    let app = test_app().await;
    let cookie = login(&app, "alice", "1234").await;
    let code = create_room(&app, &cookie).await;

    let res = post_form(&app, &cookie, &format!("/room/{code}/event"), "kind=drink").await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    post_form(&app, &cookie, &format!("/room/{code}/event"), "kind=shot").await;
    assert!(room_page_html(&app, &cookie, &code)
        .await
        .contains("1 drinks &middot; 1 shots"));

    post_form(&app, &cookie, &format!("/room/{code}/undo"), "").await;
    assert!(room_page_html(&app, &cookie, &code)
        .await
        .contains("1 drinks &middot; 0 shots"));

    // Junk kind is rejected.
    let res = post_form(&app, &cookie, &format!("/room/{code}/event"), "kind=beer").await;
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_end_room_closes_it_for_everyone() {
    let app = test_app().await;
    let cookie = login(&app, "alice", "1234").await;
    let code = create_room(&app, &cookie).await;

    let res = post_form(&app, &cookie, &format!("/room/{code}/end"), "").await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    // Room page is now a friendly 404; logging is rejected too.
    let res = app
        .clone()
        .oneshot(
            Request::get(format!("/room/{code}"))
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    let res = post_form(&app, &cookie, &format!("/room/{code}/event"), "kind=drink").await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_non_members_cannot_mutate_a_room() {
    let app = test_app().await;
    let alice = login(&app, "alice", "1234").await;
    let mallory = login(&app, "mallory", "6666").await;
    let code = create_room(&app, &alice).await;

    // Mallory has a session but never joined this room.
    let res = post_form(&app, &mallory, &format!("/room/{code}/event"), "kind=drink").await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    let res = post_form(&app, &mallory, &format!("/room/{code}/end"), "").await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // After visiting the room page (auto-join), Mallory is a member and may act.
    room_page_html(&app, &mallory, &code).await;
    let res = post_form(&app, &mallory, &format!("/room/{code}/event"), "kind=drink").await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn test_screen_is_public() {
    let app = test_app().await;
    let cookie = login(&app, "alice", "1234").await;
    let code = create_room(&app, &cookie).await;

    // No cookie at all — spectator view must render.
    let res = app
        .oneshot(
            Request::get(format!("/room/{code}/screen"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let html = body_string(res).await;
    assert!(html.contains(&code));
    assert!(html.contains("alice"));
}

#[tokio::test]
async fn test_sse_endpoint_streams_event_stream() {
    use futures::StreamExt; // for .next() on the body data stream
    let app = test_app().await;
    let cookie = login(&app, "alice", "1234").await;
    let code = create_room(&app, &cookie).await;

    let res = app
        .clone()
        .oneshot(
            Request::get(format!("/room/{code}/sse"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert!(res.headers()[header::CONTENT_TYPE]
        .to_str()
        .unwrap()
        .starts_with("text/event-stream"));
    assert_eq!(res.headers()["x-accel-buffering"], "no");
    // Don't collect the body — it's an infinite stream. Read one frame:
    let mut body = res.into_body().into_data_stream();
    let first = body.next().await.unwrap().unwrap();
    let text = String::from_utf8(first.to_vec()).unwrap();
    assert!(text.contains("event: leaderboard"));
    assert!(text.contains("alice"));

    // The initial snapshot also carries the idle game panel as a second frame.
    let game_snapshot = body.next().await.unwrap().unwrap();
    let text = String::from_utf8(game_snapshot.to_vec()).unwrap();
    assert!(text.contains("event: game"));

    // A mutation while the stream is open pushes a fresh leaderboard frame.
    let res = post_form(&app, &cookie, &format!("/room/{code}/event"), "kind=drink").await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    let second = body.next().await.unwrap().unwrap();
    let text = String::from_utf8(second.to_vec()).unwrap();
    assert!(text.contains("event: leaderboard"));
    assert!(text.contains("1 drinks"));

    // Ending the room pushes the terminal "ended" event.
    let res = post_form(&app, &cookie, &format!("/room/{code}/end"), "").await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let third = body.next().await.unwrap().unwrap();
    let text = String::from_utf8(third.to_vec()).unwrap();
    assert!(text.contains("event: ended"));
}

#[tokio::test]
async fn test_sse_on_already_ended_room_is_not_found_and_stays_closed() {
    // The subscribe-after-end race (subscribe() re-creating a hub entry that
    // end_room_handler already removed) isn't reachable through the public
    // endpoint contract — get_open_room's first check already returns 404
    // for an ended room, before any subscribe happens. This test confirms
    // that contract holds and that hitting /sse again after end never
    // resurrects a zombie hub entry that a client could still connect to.
    let app = test_app().await;
    let cookie = login(&app, "alice", "1234").await;
    let code = create_room(&app, &cookie).await;

    let res = post_form(&app, &cookie, &format!("/room/{code}/end"), "").await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    // A fresh SSE request after the room is closed must be rejected outright,
    // not silently re-open a stream.
    let res = app
        .oneshot(
            Request::get(format!("/room/{code}/sse"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_room_page_shows_idle_game_panel() {
    let app = test_app().await;
    let cookie = login(&app, "alice", "1234").await;
    let code = create_room(&app, &cookie).await;
    let html = room_page_html(&app, &cookie, &code).await;
    assert!(html.contains("Start Ring of Fire"));
    assert!(html.contains("Standard")); // seeded preset in the picker
    assert!(html.contains(r#"id="game-panel""#));
}

#[tokio::test]
async fn test_start_and_draw_flow() {
    let app = test_app().await;
    let cookie = login(&app, "alice", "1234").await;
    let code = create_room(&app, &cookie).await;

    let res = post_form(
        &app,
        &cookie,
        &format!("/room/{code}/game/start"),
        "preset_id=1",
    )
    .await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    let html = room_page_html(&app, &cookie, &code).await;
    assert!(html.contains("52 cards left"));
    assert!(html.contains("Tap to draw"));

    let res = post_form(&app, &cookie, &format!("/room/{code}/game/draw"), "").await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    let html = room_page_html(&app, &cookie, &code).await;
    assert!(html.contains("51 cards left"));
    assert!(html.contains("alice drew"));
    assert!(html.contains("card-face")); // a card is showing
}

#[tokio::test]
async fn test_game_error_paths() {
    let app = test_app().await;
    let cookie = login(&app, "alice", "1234").await;
    let code = create_room(&app, &cookie).await;

    // Draw with no active game.
    let res = post_form(&app, &cookie, &format!("/room/{code}/game/draw"), "").await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    // Unknown preset.
    let res = post_form(
        &app,
        &cookie,
        &format!("/room/{code}/game/start"),
        "preset_id=999",
    )
    .await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    // Start while a game is running.
    post_form(
        &app,
        &cookie,
        &format!("/room/{code}/game/start"),
        "preset_id=1",
    )
    .await;
    let res = post_form(
        &app,
        &cookie,
        &format!("/room/{code}/game/start"),
        "preset_id=1",
    )
    .await;
    assert_eq!(res.status(), StatusCode::CONFLICT);
    assert!(body_string(res).await.contains("already running"));
}

#[tokio::test]
async fn test_non_members_cannot_touch_the_game() {
    let app = test_app().await;
    let alice = login(&app, "alice", "1234").await;
    let mallory = login(&app, "mallory", "6666").await;
    let code = create_room(&app, &alice).await;
    for (path, body) in [
        (format!("/room/{code}/game/start"), "preset_id=1"),
        (format!("/room/{code}/game/draw"), ""),
        (format!("/room/{code}/game/spend"), "draw_id=1"),
        (format!("/room/{code}/game/end"), ""),
    ] {
        let res = post_form(&app, &mallory, &path, body).await;
        assert_eq!(res.status(), StatusCode::FORBIDDEN, "{path}");
    }
}

/// Deterministic held-card test: start the game through the db layer with a
/// crafted deck whose first card is the holdable 5 of hearts.
async fn start_rigged_game(pool: &sqlx::SqlitePool, code: &str) -> i64 {
    let room = drinkinggame::db::get_open_room(pool, code).await.unwrap();
    let mut deck = drinkinggame::cards::shuffled_deck();
    let five_pos = deck.iter().position(|c| c.rank == 5).unwrap();
    deck.swap(0, five_pos);
    drinkinggame::db::start_game(
        pool,
        room.id,
        &drinkinggame::rules::standard_rules_json(),
        &drinkinggame::cards::deck_to_string(&deck),
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn test_holdable_card_spend_flow() {
    let (app, pool) = test_app_with_pool().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await; // bob joins
    let game = start_rigged_game(&pool, &code).await;

    // Alice draws the rigged Thumb Master.
    post_form(&app, &alice, &format!("/room/{code}/game/draw"), "").await;
    let html = room_page_html(&app, &alice, &code).await;
    assert!(html.contains("held-strip"));
    assert!(html.contains("Thumb Master"));
    assert!(html.contains("use-btn"));

    let draw_id = drinkinggame::db::get_draws(&pool, game).await[0].id;
    // Bob cannot spend alice's card.
    let res = post_form(
        &app,
        &bob,
        &format!("/room/{code}/game/spend"),
        &format!("draw_id={draw_id}"),
    )
    .await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    // Alice spends it; second spend fails.
    let res = post_form(
        &app,
        &alice,
        &format!("/room/{code}/game/spend"),
        &format!("draw_id={draw_id}"),
    )
    .await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    let res = post_form(
        &app,
        &alice,
        &format!("/room/{code}/game/spend"),
        &format!("draw_id={draw_id}"),
    )
    .await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    // Held strip is gone from the page.
    assert!(!room_page_html(&app, &alice, &code)
        .await
        .contains("held-strip"));
}

#[tokio::test]
async fn test_52nd_draw_auto_ends_game() {
    let app = test_app().await;
    let cookie = login(&app, "alice", "1234").await;
    let code = create_room(&app, &cookie).await;
    post_form(
        &app,
        &cookie,
        &format!("/room/{code}/game/start"),
        "preset_id=1",
    )
    .await;
    for _ in 0..52 {
        let res = post_form(&app, &cookie, &format!("/room/{code}/game/draw"), "").await;
        assert_eq!(res.status(), StatusCode::NO_CONTENT);
    }
    // Game over: drawing again is NoActiveGame, room is idle again.
    let res = post_form(&app, &cookie, &format!("/room/{code}/game/draw"), "").await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert!(room_page_html(&app, &cookie, &code)
        .await
        .contains("Start Ring of Fire"));
}

/// Regression test: the 52nd draw must broadcast the final card BEFORE the
/// game-over summary — not end the game first and skip straight to idle.
#[tokio::test]
async fn test_52nd_draw_broadcasts_final_card_before_game_over() {
    use futures::StreamExt;
    let app = test_app().await;
    let cookie = login(&app, "alice", "1234").await;
    let code = create_room(&app, &cookie).await;
    post_form(
        &app,
        &cookie,
        &format!("/room/{code}/game/start"),
        "preset_id=1",
    )
    .await;
    for _ in 0..51 {
        post_form(&app, &cookie, &format!("/room/{code}/game/draw"), "").await;
    }

    // Subscribe before the 52nd (final) draw.
    let res = app
        .clone()
        .oneshot(
            Request::get(format!("/room/{code}/sse"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let mut body = res.into_body().into_data_stream();
    // Drain the initial two-frame snapshot (leaderboard, then game).
    body.next().await.unwrap().unwrap();
    body.next().await.unwrap().unwrap();

    let res = post_form(&app, &cookie, &format!("/room/{code}/game/draw"), "").await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    // First pushed frame: the active panel showing the 52nd card, not idle.
    let first = String::from_utf8(body.next().await.unwrap().unwrap().to_vec()).unwrap();
    assert!(first.contains("event: game"));
    assert!(first.contains("0 cards left"));
    assert!(first.contains("card-face"));
    assert!(!first.contains("Start Ring of Fire"));

    // Second pushed frame: the game-over summary followed by the idle panel.
    let second = String::from_utf8(body.next().await.unwrap().unwrap().to_vec()).unwrap();
    assert!(second.contains("event: game"));
    assert!(second.contains("Game over"));
    assert!(second.contains("Start Ring of Fire"));
}

#[tokio::test]
async fn test_end_game_early() {
    let app = test_app().await;
    let cookie = login(&app, "alice", "1234").await;
    let code = create_room(&app, &cookie).await;
    post_form(
        &app,
        &cookie,
        &format!("/room/{code}/game/start"),
        "preset_id=1",
    )
    .await;
    post_form(&app, &cookie, &format!("/room/{code}/game/draw"), "").await;
    let res = post_form(&app, &cookie, &format!("/room/{code}/game/end"), "").await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    assert!(room_page_html(&app, &cookie, &code)
        .await
        .contains("Start Ring of Fire"));
}

#[tokio::test]
async fn test_screen_and_sse_carry_game_panel() {
    use futures::StreamExt;
    let app = test_app().await;
    let cookie = login(&app, "alice", "1234").await;
    let code = create_room(&app, &cookie).await;
    post_form(
        &app,
        &cookie,
        &format!("/room/{code}/game/start"),
        "preset_id=1",
    )
    .await;

    // Spectator page renders the panel server-side.
    let res = app
        .clone()
        .oneshot(
            Request::get(format!("/room/{code}/screen"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(body_string(res).await.contains("52 cards left"));

    // SSE: initial game snapshot, then a draw pushes a fresh panel.
    let res = app
        .clone()
        .oneshot(
            Request::get(format!("/room/{code}/sse"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let mut body = res.into_body().into_data_stream();
    let first = String::from_utf8(body.next().await.unwrap().unwrap().to_vec()).unwrap();
    assert!(first.contains("event: leaderboard"));
    let second = String::from_utf8(body.next().await.unwrap().unwrap().to_vec()).unwrap();
    assert!(second.contains("event: game"));
    assert!(second.contains("52 cards left"));

    post_form(&app, &cookie, &format!("/room/{code}/game/draw"), "").await;
    let third = String::from_utf8(body.next().await.unwrap().unwrap().to_vec()).unwrap();
    assert!(third.contains("event: game"));
    assert!(third.contains("51 cards left"));
}
