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

/// Same as `test_app`, but mounted under a non-empty base path — used to
/// exercise `next`/redirect string composition against a realistic prefix
/// (production mounts the crate at "/drinks", not "").
async fn test_app_with_base(base_path: &str) -> Router {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    drinkinggame::db::run_migrations(&pool).await;
    drinkinggame::router_with_pool(pool, base_path)
}

async fn body_string(res: axum::response::Response) -> String {
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    String::from_utf8(bytes.to_vec()).unwrap()
}

/// GET helper for tests that only need the response (status/headers), not a
/// pre-built app-with-session flow.
async fn get(app: &Router, path: &str) -> axum::response::Response {
    app.clone()
        .oneshot(Request::get(path).body(Body::empty()).unwrap())
        .await
        .unwrap()
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
    assert!(html.contains("LET'S GO"));
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

#[tokio::test]
async fn test_font_and_sound_routes() {
    let app = test_app().await;
    // Fonts embedded — always 200 with the right type.
    let res = get(&app, "/assets/fonts/archivo-800.woff2").await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.headers()["content-type"], "font/woff2");
    // Unknown font name → 404, no traversal.
    assert_eq!(
        get(&app, "/assets/fonts/../../etc/passwd").await.status(),
        StatusCode::NOT_FOUND
    );
    // Sounds: allowlisted name but no file on disk → 404 (drop-in dir ships empty).
    assert_eq!(
        get(&app, "/assets/sounds/drink.mp3").await.status(),
        StatusCode::NOT_FOUND
    );
    // Non-allowlisted name → 404 even if a file existed.
    assert_eq!(
        get(&app, "/assets/sounds/evil.sh").await.status(),
        StatusCode::NOT_FOUND
    );
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
    assert!(html.contains("0 drinks"));
    assert!(html.contains("0 shots"));
    assert!(html.contains("0 nights"));
    assert!(html.contains("0 King's Cups"));
}

/// Lifetime nights (distinct rooms joined) and Kings (rank-13 draws) both
/// come from Task 1's db queries — this proves the landing page actually
/// wires them in rather than always showing zero.
#[tokio::test]
async fn test_landing_shows_lifetime_nights_and_kings() {
    let (app, pool) = test_app_with_pool().await;
    let cookie = login(&app, "alice", "1234").await;
    let code = create_room(&app, &cookie).await; // 1 night
    start_rigged_game_with_rank(&pool, &code, 13).await;
    post_form(&app, &cookie, &format!("/room/{code}/game/draw"), "").await; // 1 King

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
    assert!(html.contains("1 nights"));
    assert!(html.contains("1 King's Cups"));
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
    assert!(html.contains("+1 DRINK"));
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

/// QR-scan flow: a visitor who opens a room link cold (no session cookie)
/// gets bounced to login carrying `next`, not just dumped on a bare landing
/// page that forgets which room they were headed to.
#[tokio::test]
async fn test_unauthenticated_room_visit_redirects_with_next() {
    let app = test_app().await;
    let res = app
        .oneshot(Request::get("/room/QKAM").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(res.headers()[header::LOCATION], "/?next=/room/QKAM");
}

/// Same redirect, mounted at a non-empty base path — production runs the
/// crate nested under "/drinks", not standalone at "".
#[tokio::test]
async fn test_unauthenticated_room_visit_redirect_honors_base_path() {
    let app = test_app_with_base("/drinks").await;
    let res = app
        .oneshot(Request::get("/room/QKAM").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        res.headers()[header::LOCATION],
        "/drinks/?next=/drinks/room/QKAM"
    );
}

#[tokio::test]
async fn test_login_honors_valid_next_and_ignores_bad_next() {
    let app = test_app().await;
    let cookie = login(&app, "alice", "1234").await;
    let code = create_room(&app, &cookie).await;

    // A valid `next` (the room just created) takes the freshly logged-in
    // player straight into the room instead of the landing page.
    let res = app
        .clone()
        .oneshot(
            Request::post("/login")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(format!("name=bob&pin=5678&next=/room/{code}")))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(res.headers()[header::LOCATION], format!("/room/{code}"));

    // An invalid `next` (open-redirect attempt) is ignored — falls back home.
    let res = app
        .oneshot(
            Request::post("/login")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(
                    "name=mallory&pin=1111&next=https://evil.example/x",
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(res.headers()[header::LOCATION], "/");
}

/// The landing page only echoes `next` back into the hidden login field when
/// it's a validated room destination — never an open-redirect payload.
#[tokio::test]
async fn test_landing_echoes_valid_next_and_drops_bad_next() {
    let app = test_app().await;
    let res = app
        .clone()
        .oneshot(
            Request::get("/?next=/room/QKAM")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(body_string(res)
        .await
        .contains(r#"<input type="hidden" name="next" value="/room/QKAM">"#));

    let res = app
        .oneshot(
            Request::get("/?next=https://evil.example/x")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(!body_string(res).await.contains("evil.example"));
}

/// Full seam, end to end, under a non-empty base path: the exact `Location`
/// the rejection redirect produces is fed straight back into the landing
/// GET, and the hidden field it renders is the exact value `login` will
/// later accept. Each half is covered separately elsewhere; this is the one
/// test that proves the value flowing out of one half is the value the
/// other half consumes, rather than two halves that merely look compatible.
#[tokio::test]
async fn test_qr_round_trip_composes_end_to_end_under_base_path() {
    let app = test_app_with_base("/drinks").await;

    // Cold visit to a room link with no session.
    let res = app
        .clone()
        .oneshot(Request::get("/room/QKAM").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let location = res.headers()[header::LOCATION].to_str().unwrap();
    assert_eq!(location, "/drinks/?next=/drinks/room/QKAM");

    // Follow it exactly as a browser would — strip the mount prefix the way
    // nest_service does, since this router isn't actually nested here.
    let landing_path = location.strip_prefix("/drinks").unwrap();
    let res = app
        .clone()
        .oneshot(Request::get(landing_path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let html = body_string(res).await;
    assert!(html.contains(r#"<input type="hidden" name="next" value="/drinks/room/QKAM">"#));

    // Logging in with that exact hidden-field value lands back in the room.
    let res = app
        .oneshot(
            Request::post("/login")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from("name=carol&pin=2468&next=/drinks/room/QKAM"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(res.headers()[header::LOCATION], "/drinks/room/QKAM");
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
        .contains("1 D &middot; 1 S"));

    post_form(&app, &cookie, &format!("/room/{code}/undo"), "").await;
    assert!(room_page_html(&app, &cookie, &code)
        .await
        .contains("1 D &middot; 0 S"));

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
    assert!(html.contains("qr-box"));
    // Spectator surface — no navigable link back into the phone-only preset
    // editor (regression coverage for the leak an earlier review caught).
    assert!(!html.contains("presets-link"));
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

    // The initial snapshot also carries the idle game panel as a second
    // frame, then the screen and room panels as a third and fourth.
    let game_snapshot = body.next().await.unwrap().unwrap();
    let text = String::from_utf8(game_snapshot.to_vec()).unwrap();
    assert!(text.contains("event: game"));
    let screen_snapshot = body.next().await.unwrap().unwrap();
    assert!(String::from_utf8(screen_snapshot.to_vec())
        .unwrap()
        .contains("event: screen"));
    let room_snapshot = body.next().await.unwrap().unwrap();
    assert!(String::from_utf8(room_snapshot.to_vec())
        .unwrap()
        .contains("event: room"));

    // A mutation while the stream is open pushes a fresh leaderboard frame,
    // followed by an emote (self-logged drinks always fire one).
    let res = post_form(&app, &cookie, &format!("/room/{code}/event"), "kind=drink").await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    let second = body.next().await.unwrap().unwrap();
    let text = String::from_utf8(second.to_vec()).unwrap();
    assert!(text.contains("event: leaderboard"));
    assert!(text.contains("1 D &middot;"));
    let emote = body.next().await.unwrap().unwrap();
    assert!(String::from_utf8(emote.to_vec())
        .unwrap()
        .contains("event: emote"));

    // Ending the room pushes the terminal "ended" event.
    let res = post_form(&app, &cookie, &format!("/room/{code}/end"), "").await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let third = body.next().await.unwrap().unwrap();
    let text = String::from_utf8(third.to_vec()).unwrap();
    assert!(text.contains("event: ended"));
    // A named SSE event with no `data:` field is silently dropped by the
    // browser's EventSource parser (WHATWG SSE spec: if the data buffer is
    // empty when a blank line is reached, no event fires at all) — so
    // `event: ended` alone isn't enough; the frame must carry a data line
    // or the client-side `es.addEventListener("ended", ...)` handler in
    // room.html/screen.html never runs and the tab never redirects.
    assert!(
        text.contains("data:"),
        "ended frame has no data: field, EventSource will drop it silently: {text:?}"
    );
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
    assert!(html.contains(">START<"));
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
    assert!(html.contains("52 LEFT"));
    assert!(html.contains("TAP TO DRAW"));

    let res = post_form(&app, &cookie, &format!("/room/{code}/game/draw"), "").await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    let html = room_page_html(&app, &cookie, &code).await;
    assert!(html.contains("51 LEFT"));
    assert!(html.contains("alice DREW"));
    assert!(html.contains("card-big")); // a card is showing
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
        (format!("/room/{code}/game/rule"), "text=x"),
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
        "ring_of_fire",
        &drinkinggame::rules::standard_rules_json(),
        &drinkinggame::cards::deck_to_string(&deck),
        None,
    )
    .await
    .unwrap()
}

/// Same as `start_rigged_game` but the rigged top card is any rank.
async fn start_rigged_game_with_rank(pool: &sqlx::SqlitePool, code: &str, rank: u8) -> i64 {
    let room = drinkinggame::db::get_open_room(pool, code).await.unwrap();
    let mut deck = drinkinggame::cards::shuffled_deck();
    let pos = deck.iter().position(|c| c.rank == rank).unwrap();
    deck.swap(0, pos);
    drinkinggame::db::start_game(
        pool,
        room.id,
        "ring_of_fire",
        &drinkinggame::rules::standard_rules_json(),
        &drinkinggame::cards::deck_to_string(&deck),
        None,
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn test_jack_rule_flow() {
    use futures::StreamExt;
    let (app, pool) = test_app_with_pool().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await; // bob joins
    let game = start_rigged_game_with_rank(&pool, &code, 11).await;

    // Alice draws the rigged Jack.
    post_form(&app, &alice, &format!("/room/{code}/game/draw"), "").await;

    // Subscribe so the rule POST's broadcasts can be observed directly —
    // room.html doesn't render the ROOM/TABLE panel yet (that shell lands in
    // a later task), so the SSE `room` event is the only reachable surface
    // for "the rule reaches viewers" today.
    let sse_res = app
        .clone()
        .oneshot(
            Request::get(format!("/room/{code}/sse"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let mut sse_body = sse_res.into_body().into_data_stream();
    for _ in 0..4 {
        sse_body.next().await.unwrap().unwrap(); // drain the 4-frame snapshot
    }

    let res = post_form(
        &app,
        &alice,
        &format!("/room/{code}/game/rule"),
        "text=No names",
    )
    .await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    // The rule is persisted...
    let rules = drinkinggame::db::house_rules(&pool, game).await;
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].text, "No names");
    // ...and broadcast: rule_handler publishes Room then Game.
    let room_frame = String::from_utf8(sse_body.next().await.unwrap().unwrap().to_vec()).unwrap();
    assert!(room_frame.contains("event: room"));
    assert!(room_frame.contains("No names"));
    let game_frame = String::from_utf8(sse_body.next().await.unwrap().unwrap().to_vec()).unwrap();
    assert!(game_frame.contains("event: game"));
    assert!(game_frame.contains("made a rule"));

    // Second POST for the same draw: the rule is already set.
    let res = post_form(
        &app,
        &alice,
        &format!("/room/{code}/game/rule"),
        "text=Another+rule",
    )
    .await;
    assert_eq!(res.status(), StatusCode::CONFLICT);

    // Bob is not the drawer.
    let res = post_form(
        &app,
        &bob,
        &format!("/room/{code}/game/rule"),
        "text=Bob%27s+rule",
    )
    .await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // 201-char text is rejected.
    let long_text = "a".repeat(201);
    let res = post_form(
        &app,
        &alice,
        &format!("/room/{code}/game/rule"),
        &format!("text={long_text}"),
    )
    .await;
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_rule_rejected_when_latest_draw_not_jack() {
    let (app, pool) = test_app_with_pool().await;
    let cookie = login(&app, "alice", "1234").await;
    let code = create_room(&app, &cookie).await;
    start_rigged_game_with_rank(&pool, &code, 5).await; // Thumb Master, not a Jack
    post_form(&app, &cookie, &format!("/room/{code}/game/draw"), "").await;
    let res = post_form(
        &app,
        &cookie,
        &format!("/room/{code}/game/rule"),
        "text=Nope",
    )
    .await;
    assert_eq!(res.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn test_rof_routes_reject_three_man_games() {
    let (app, pool) = test_app_with_pool().await;
    let cookie = login(&app, "alice", "1234").await;
    let code = create_room(&app, &cookie).await;
    let room = drinkinggame::db::get_open_room(&pool, &code).await.unwrap();
    sqlx::query(
        "INSERT INTO games (room_id, kind, rules_json, deck_order, state_json)
         VALUES (?1, 'three_man', '[]', '', NULL)",
    )
    .bind(room.id)
    .execute(&pool)
    .await
    .unwrap();

    for (path, body) in [
        (format!("/room/{code}/game/draw"), ""),
        (format!("/room/{code}/game/spend"), "draw_id=1"),
        (format!("/room/{code}/game/rule"), "text=x"),
        (format!("/room/{code}/game/end"), ""),
    ] {
        let res = post_form(&app, &cookie, &path, body).await;
        assert_eq!(res.status(), StatusCode::CONFLICT, "{path}");
    }
}

#[tokio::test]
async fn test_sse_snapshot_has_all_stateful_kinds() {
    use futures::StreamExt;
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
    let mut body = res.into_body().into_data_stream();
    let mut seen = std::collections::HashSet::new();
    for _ in 0..4 {
        let frame = String::from_utf8(body.next().await.unwrap().unwrap().to_vec()).unwrap();
        for name in ["leaderboard", "game", "screen", "room"] {
            if frame.contains(&format!("event: {name}")) {
                seen.insert(name);
            }
        }
    }
    assert_eq!(
        seen.len(),
        4,
        "expected all 4 snapshot events, got {seen:?}"
    );
}

#[tokio::test]
async fn test_event_broadcasts_emote_and_room_join_broadcasts_room() {
    use futures::StreamExt;
    let app = test_app().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let code = create_room(&app, &alice).await;

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
    // Drain the 4-frame snapshot.
    for _ in 0..4 {
        body.next().await.unwrap().unwrap();
    }

    // Logging a drink broadcasts both a leaderboard refresh and an emote.
    let res = post_form(&app, &alice, &format!("/room/{code}/event"), "kind=drink").await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    let mut got_emote = false;
    let mut got_leaderboard = false;
    for _ in 0..2 {
        let frame = String::from_utf8(body.next().await.unwrap().unwrap().to_vec()).unwrap();
        if frame.contains("event: emote") {
            got_emote = true;
            assert!(frame.contains("🍺"));
        }
        if frame.contains("event: leaderboard") {
            got_leaderboard = true;
        }
    }
    assert!(got_emote, "expected an emote event");
    assert!(got_leaderboard, "expected a leaderboard event");

    // Undo broadcasts a leaderboard refresh but never an emote.
    let res = post_form(&app, &alice, &format!("/room/{code}/undo"), "").await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    let frame = String::from_utf8(body.next().await.unwrap().unwrap().to_vec()).unwrap();
    assert!(frame.contains("event: leaderboard"));

    // A second player joining broadcasts a Room message.
    room_page_html(&app, &bob, &code).await;
    let frame = String::from_utf8(body.next().await.unwrap().unwrap().to_vec()).unwrap();
    assert!(frame.contains("event: room"));
    assert!(frame.contains("bob"));
}

#[tokio::test]
async fn test_end_night_ends_game_and_room() {
    let (app, pool) = test_app_with_pool().await;
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

    let room = drinkinggame::db::get_open_room(&pool, &code).await.unwrap();
    let game = drinkinggame::db::get_active_game(&pool, room.id)
        .await
        .unwrap();

    let res = post_form(&app, &cookie, &format!("/room/{code}/end"), "").await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    // Both the room and its active game are now ended.
    assert!(drinkinggame::db::get_open_room(&pool, &code)
        .await
        .is_none());
    let ended_game =
        sqlx::query_as::<_, (Option<String>,)>("SELECT ended_at FROM games WHERE id = ?1")
            .bind(game.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(ended_game.0.is_some());
}

/// start_game_handler calls broadcast_room after broadcast_game so the
/// ROOM/TABLE tab's data-mode flips idle -> ring_of_fire immediately,
/// instead of waiting for the next unrelated room event.
#[tokio::test]
async fn test_start_game_broadcasts_room_mode_flip() {
    use futures::StreamExt;
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
    let mut body = res.into_body().into_data_stream();
    for _ in 0..4 {
        body.next().await.unwrap().unwrap(); // drain the idle-mode snapshot
    }

    post_form(
        &app,
        &cookie,
        &format!("/room/{code}/game/start"),
        "preset_id=1",
    )
    .await;
    // broadcast_game publishes Game then Screen; broadcast_room follows.
    let game_frame = String::from_utf8(body.next().await.unwrap().unwrap().to_vec()).unwrap();
    assert!(game_frame.contains("event: game"));
    let screen_frame = String::from_utf8(body.next().await.unwrap().unwrap().to_vec()).unwrap();
    assert!(screen_frame.contains("event: screen"));
    let room_frame = String::from_utf8(body.next().await.unwrap().unwrap().to_vec()).unwrap();
    assert!(room_frame.contains("event: room"));
    assert!(room_frame.contains(r#"data-mode="ring_of_fire""#));
}

/// draw_handler refreshes the ROOM/TABLE tab whenever the drawn card is a
/// King, since that's the only draw outcome that changes the King's Cup fill.
#[tokio::test]
async fn test_king_draw_broadcasts_room() {
    use futures::StreamExt;
    let (app, pool) = test_app_with_pool().await;
    let cookie = login(&app, "alice", "1234").await;
    let code = create_room(&app, &cookie).await;
    start_rigged_game_with_rank(&pool, &code, 13).await;

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
    for _ in 0..4 {
        body.next().await.unwrap().unwrap(); // drain the snapshot
    }

    post_form(&app, &cookie, &format!("/room/{code}/game/draw"), "").await;
    // broadcast_game publishes Game then Screen; the King-fill broadcast_room follows.
    let game_frame = String::from_utf8(body.next().await.unwrap().unwrap().to_vec()).unwrap();
    assert!(game_frame.contains("event: game"));
    let screen_frame = String::from_utf8(body.next().await.unwrap().unwrap().to_vec()).unwrap();
    assert!(screen_frame.contains("event: screen"));
    let room_frame = String::from_utf8(body.next().await.unwrap().unwrap().to_vec()).unwrap();
    assert!(room_frame.contains("event: room"));
    assert!(room_frame.contains("1 / 4"));
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
        .contains(">START<"));
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
    // Drain the initial four-frame snapshot (leaderboard, game, screen, room).
    for _ in 0..4 {
        body.next().await.unwrap().unwrap();
    }

    let res = post_form(&app, &cookie, &format!("/room/{code}/game/draw"), "").await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    // First pushed frame: the phone active panel showing the 52nd card, not idle.
    let first = String::from_utf8(body.next().await.unwrap().unwrap().to_vec()).unwrap();
    assert!(first.contains("event: game"));
    assert!(first.contains("0 LEFT"));
    assert!(first.contains("card-big"));
    assert!(!first.contains(">START<"));

    // Second pushed frame: the screen active panel, same card.
    let screen_active = String::from_utf8(body.next().await.unwrap().unwrap().to_vec()).unwrap();
    assert!(screen_active.contains("event: screen"));
    assert!(screen_active.contains("0 of 52 left"));

    // Third pushed frame: the phone game-over summary followed by the idle panel.
    let third = String::from_utf8(body.next().await.unwrap().unwrap().to_vec()).unwrap();
    assert!(third.contains("event: game"));
    assert!(third.contains("GAME OVER"));
    assert!(third.contains(">START<"));

    // Fourth pushed frame: the screen's game-over panel.
    let screen_over = String::from_utf8(body.next().await.unwrap().unwrap().to_vec()).unwrap();
    assert!(screen_over.contains("event: screen"));

    // Fifth pushed frame: the room panel refresh (king-fill reset).
    let room_frame = String::from_utf8(body.next().await.unwrap().unwrap().to_vec()).unwrap();
    assert!(room_frame.contains("event: room"));
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
        .contains(">START<"));
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

    // Spectator page renders the screen-scale panel server-side — not the
    // phone's "52 LEFT" deck-row, which belongs to the GAME tab only.
    let res = app
        .clone()
        .oneshot(
            Request::get(format!("/room/{code}/screen"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(body_string(res).await.contains("52 of 52 left"));

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
    assert!(second.contains("52 LEFT"));
    // Snapshot also carries the screen and room panels as a third and fourth frame.
    let screen_snapshot = String::from_utf8(body.next().await.unwrap().unwrap().to_vec()).unwrap();
    assert!(screen_snapshot.contains("event: screen"));
    let room_snapshot = String::from_utf8(body.next().await.unwrap().unwrap().to_vec()).unwrap();
    assert!(room_snapshot.contains("event: room"));

    post_form(&app, &cookie, &format!("/room/{code}/game/draw"), "").await;
    let fifth = String::from_utf8(body.next().await.unwrap().unwrap().to_vec()).unwrap();
    assert!(fifth.contains("event: game"));
    assert!(fifth.contains("51 LEFT"));
}

#[tokio::test]
async fn test_presets_require_login() {
    let app = test_app().await;
    let res = app
        .oneshot(Request::get("/presets").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER); // PlayerSession redirect
}

#[tokio::test]
async fn test_presets_list_and_create_copy() {
    let app = test_app().await;
    let cookie = login(&app, "alice", "1234").await;
    let res = app
        .clone()
        .oneshot(
            Request::get("/presets")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert!(body_string(res).await.contains("Standard"));

    // Create a copy of Standard.
    let res = post_form(&app, &cookie, "/presets", "name=House&source_id=1").await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let loc = res.headers()[header::LOCATION]
        .to_str()
        .unwrap()
        .to_string();
    assert!(loc.starts_with("/presets/"));

    // Edit page shows the copied rules.
    let res = app
        .clone()
        .oneshot(
            Request::get(&loc)
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let html = body_string(res).await;
    assert!(html.contains("House"));
    assert!(html.contains("Waterfall"));

    // Duplicate name is a friendly conflict.
    let res = post_form(&app, &cookie, "/presets", "name=House&source_id=1").await;
    assert_eq!(res.status(), StatusCode::CONFLICT);
}

/// Builds the full 13-rank save body from the standard rules, with one
/// override applied.
fn edit_body(name: &str, override_rank: u8, new_title: &str) -> String {
    let mut parts = vec![format!("name={name}")];
    for r in drinkinggame::rules::standard_rules() {
        let title = if r.rank == override_rank {
            new_title
        } else {
            &r.title
        };
        parts.push(format!("title_{}={}", r.rank, urlencode(title)));
        parts.push(format!("text_{}={}", r.rank, urlencode(&r.text)));
        if r.holdable {
            parts.push(format!("holdable_{}=on", r.rank));
        }
    }
    parts.join("&")
}

/// Minimal urlencoding for test bodies (spaces and ampersands only —
/// standard rule text contains no other reserved characters).
fn urlencode(s: &str) -> String {
    s.replace('%', "%25")
        .replace('&', "%26")
        .replace('+', "%2B")
        .replace(' ', "+")
}

#[tokio::test]
async fn test_preset_save_and_delete() {
    let (app, pool) = test_app_with_pool().await;
    let cookie = login(&app, "alice", "1234").await;
    let res = post_form(&app, &cookie, "/presets", "name=House&source_id=1").await;
    let loc = res.headers()[header::LOCATION]
        .to_str()
        .unwrap()
        .to_string();
    let id: i64 = loc.rsplit('/').next().unwrap().parse().unwrap();

    // Save with rank 4 renamed.
    let res = post_form(&app, &cookie, &loc, &edit_body("House", 4, "Floor")).await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let saved = drinkinggame::db::get_preset(&pool, id).await.unwrap();
    let rules = drinkinggame::rules::parse_rules(&saved.rules_json);
    assert_eq!(drinkinggame::rules::rule_for_rank(&rules, 4).title, "Floor");
    assert!(drinkinggame::rules::rule_for_rank(&rules, 5).holdable); // survives roundtrip

    // Delete — including that deleting is allowed for any preset.
    let res = post_form(&app, &cookie, &format!("{loc}/delete"), "").await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert!(drinkinggame::db::get_preset(&pool, id).await.is_none());
}

#[tokio::test]
async fn test_running_game_unaffected_by_preset_edit() {
    let (app, pool) = test_app_with_pool().await;
    let cookie = login(&app, "alice", "1234").await;
    let code = create_room(&app, &cookie).await;
    post_form(
        &app,
        &cookie,
        &format!("/room/{code}/game/start"),
        "preset_id=1",
    )
    .await;
    // Mutate Standard after the game started.
    post_form(
        &app,
        &cookie,
        "/presets/1",
        &edit_body("Standard", 1, "Tsunami"),
    )
    .await;
    // The running game still holds the snapshot.
    let room = drinkinggame::db::get_open_room(&pool, &code).await.unwrap();
    let game = drinkinggame::db::get_active_game(&pool, room.id)
        .await
        .unwrap();
    let rules = drinkinggame::rules::parse_rules(&game.rules_json);
    assert_eq!(
        drinkinggame::rules::rule_for_rank(&rules, 1).title,
        "Waterfall"
    );
}

// -------------------------------------------------------------
// 3 Man (Task 12): /tm/start, /tm/end, idle-panel start card,
// kind-dispatched panels.
// -------------------------------------------------------------

#[tokio::test]
async fn test_tm_start_seeds_order_and_renders_dice_ui() {
    let (app, pool) = test_app_with_pool().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await; // bob joins

    let res = post_form(&app, &alice, &format!("/room/{code}/tm/start"), "").await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    let room = drinkinggame::db::get_open_room(&pool, &code).await.unwrap();
    let game = drinkinggame::db::get_active_game(&pool, room.id)
        .await
        .unwrap();
    assert_eq!(game.kind, "three_man");
    assert_eq!(game.deck_order, "");
    assert_eq!(game.rules_json, "");
    assert!(game.state_json.is_some());

    let html = room_page_html(&app, &alice, &code).await;
    assert!(html.contains("ROLL THE DICE"));
    // alice created the room, so she's the starter and initial 3 Man.
    let alice_player = drinkinggame::db::get_player_by_name(&pool, "alice")
        .await
        .unwrap();
    assert!(html.contains(&format!(r#"data-three-man="{}""#, alice_player.id)));
}

#[tokio::test]
async fn test_tm_start_needs_two_players() {
    let app = test_app().await;
    let alice = login(&app, "alice", "1234").await;
    let code = create_room(&app, &alice).await;

    let res = post_form(&app, &alice, &format!("/room/{code}/tm/start"), "").await;
    assert_eq!(res.status(), StatusCode::CONFLICT);
    assert!(body_string(res).await.contains("at least 2 players"));
}

#[tokio::test]
async fn test_tm_start_conflicts_with_active_rof() {
    let app = test_app().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;
    post_form(
        &app,
        &alice,
        &format!("/room/{code}/game/start"),
        "preset_id=1",
    )
    .await;

    let res = post_form(&app, &alice, &format!("/room/{code}/tm/start"), "").await;
    assert_eq!(res.status(), StatusCode::CONFLICT);
    assert!(body_string(res).await.contains("already running"));
}

/// `/tm/roll` (and the rest of the action routes) belong to a later task —
/// `/tm/end` is the only other 3 Man route this task registers, and it's
/// kind-gated through the same `load_tm()` helper every future `/tm/*`
/// action handler will share, so it exercises the WrongGameKind path those
/// routes will need without requiring routes this task doesn't build.
#[tokio::test]
async fn test_tm_routes_reject_rof_games() {
    let app = test_app().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;
    post_form(
        &app,
        &alice,
        &format!("/room/{code}/game/start"),
        "preset_id=1",
    )
    .await;

    let res = post_form(&app, &alice, &format!("/room/{code}/tm/end"), "").await;
    assert_eq!(res.status(), StatusCode::CONFLICT);
    assert!(body_string(res).await.contains("belongs to the other game"));
}

#[tokio::test]
async fn test_tm_end_broadcasts_summary_and_idle() {
    use futures::StreamExt;
    let (app, pool) = test_app_with_pool().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;
    post_form(&app, &alice, &format!("/room/{code}/tm/start"), "").await;

    // alice: 3 drinks, 1 shot. bob: 1 drink, 2 shots.
    for _ in 0..3 {
        post_form(&app, &alice, &format!("/room/{code}/event"), "kind=drink").await;
    }
    post_form(&app, &alice, &format!("/room/{code}/event"), "kind=shot").await;
    post_form(&app, &bob, &format!("/room/{code}/event"), "kind=drink").await;
    for _ in 0..2 {
        post_form(&app, &bob, &format!("/room/{code}/event"), "kind=shot").await;
    }

    let sse_res = app
        .clone()
        .oneshot(
            Request::get(format!("/room/{code}/sse"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let mut sse_body = sse_res.into_body().into_data_stream();
    for _ in 0..4 {
        sse_body.next().await.unwrap().unwrap(); // drain the 4-frame snapshot
    }

    let res = post_form(&app, &alice, &format!("/room/{code}/tm/end"), "").await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    let room = drinkinggame::db::get_open_room(&pool, &code).await.unwrap();
    assert!(drinkinggame::db::get_active_game(&pool, room.id)
        .await
        .is_none()); // games.ended_at is now set

    let game_frame = String::from_utf8(sse_body.next().await.unwrap().unwrap().to_vec()).unwrap();
    assert!(game_frame.contains("event: game"));
    assert!(game_frame.contains("GAME OVER"));
    assert!(game_frame.contains("HARDEST HIT"));
    assert!(game_frame.contains("alice"));
    assert!(game_frame.contains("MOST SHOTS"));
    assert!(game_frame.contains("bob"));
    // Room total is drinks + shots summed across everyone (alice 3+1, bob
    // 1+2 = 7 total) — matching Ring of Fire's game_summary/"drinks
    // logged" convention. (The brief's Step 1 sketch names "4", which is
    // drinks-only; Step 3's explicit "summed drinks + shots" instruction
    // governs, so this asserts the rendered cell rather than a bare
    // substring that both interpretations would satisfy.)
    assert!(game_frame.contains(
        r#"<span class="superla-label">ROOM TOTAL</span><span class="superla-name">7</span>"#
    ));

    let screen_frame = String::from_utf8(sse_body.next().await.unwrap().unwrap().to_vec()).unwrap();
    assert!(screen_frame.contains("event: screen"));
    assert!(screen_frame.contains("lost"));

    // Page reload shows both start cards again — the idle panel is back.
    let html = room_page_html(&app, &alice, &code).await;
    assert!(html.contains("Ring of Fire"));
    assert!(html.contains("start-card-amber"));
    assert!(html.contains("3 Man"));
}

#[tokio::test]
async fn test_idle_panel_offers_both_games() {
    let app = test_app().await;
    let alice = login(&app, "alice", "1234").await;
    let code = create_room(&app, &alice).await;

    let html = room_page_html(&app, &alice, &code).await;
    assert!(html.contains(">START<"));
    assert!(html.contains("Ring of Fire"));
    assert!(html.contains("start-card-amber"));
    assert!(html.contains("3 Man"));
    assert!(html.contains(&format!("/room/{code}/tm/start")));
}

// -------------------------------------------------------------
// 3 Man action routes (Task 13): roll/pass/three-man/mode/target/
// clear-slot/send/gift-roll/seat — per-room lock, actor gating, auto-log.
// -------------------------------------------------------------

use drinkinggame::three_man::{GiveMode, Phase, ThreeManState};

async fn tm_game_id(pool: &sqlx::SqlitePool, code: &str) -> i64 {
    let room = drinkinggame::db::get_open_room(pool, code).await.unwrap();
    drinkinggame::db::get_active_game(pool, room.id)
        .await
        .unwrap()
        .id
}

async fn tm_state(pool: &sqlx::SqlitePool, code: &str) -> ThreeManState {
    let room = drinkinggame::db::get_open_room(pool, code).await.unwrap();
    let game = drinkinggame::db::get_active_game(pool, room.id)
        .await
        .unwrap();
    ThreeManState::from_json(game.state_json.as_deref().unwrap_or_default())
}

async fn tm_drinks(pool: &sqlx::SqlitePool, code: &str, name: &str) -> i64 {
    let room = drinkinggame::db::get_open_room(pool, code).await.unwrap();
    let player = drinkinggame::db::get_player_by_name(pool, name)
        .await
        .unwrap();
    drinkinggame::db::leaderboard(pool, room.id)
        .await
        .into_iter()
        .find(|r| r.id == player.id)
        .unwrap()
        .drinks
}

/// Drives the state machine back to `Ready` without producing any drink
/// calls (calls only ever originate inside `roll()`), so the roll-until-a-
/// call-fires loop in `test_tm_roll_any_member_and_autologs` can safely
/// resolve a boring roll and try again.
async fn tm_resolve_to_ready(
    app: &Router,
    pool: &sqlx::SqlitePool,
    code: &str,
    alice: &str,
    bob: &str,
    alice_id: i64,
    bob_id: i64,
) {
    loop {
        let st = tm_state(pool, code).await;
        match st.phase {
            Phase::Ready => return,
            Phase::Rolled => {
                post_form(app, alice, &format!("/room/{code}/tm/pass"), "").await;
            }
            Phase::HandOff => {
                let roller_cookie = if st.roller() == alice_id { alice } else { bob };
                let target = if st.three_man == alice_id {
                    bob_id
                } else {
                    alice_id
                };
                post_form(
                    app,
                    roller_cookie,
                    &format!("/room/{code}/tm/three-man"),
                    &format!("target={target}"),
                )
                .await;
            }
            Phase::Assign => {
                let double = st.double.as_ref().unwrap();
                let owner_cookie = if double.owner == alice_id { alice } else { bob };
                let other = if double.owner == alice_id {
                    bob_id
                } else {
                    alice_id
                };
                post_form(
                    app,
                    owner_cookie,
                    &format!("/room/{code}/tm/mode"),
                    "mode=both",
                )
                .await;
                post_form(
                    app,
                    owner_cookie,
                    &format!("/room/{code}/tm/target"),
                    &format!("slot=0&target={other}"),
                )
                .await;
                post_form(app, owner_cookie, &format!("/room/{code}/tm/send"), "").await;
            }
            Phase::Gifts => {
                if st.gifts_complete() {
                    post_form(app, alice, &format!("/room/{code}/tm/pass"), "").await;
                } else {
                    post_form(app, alice, &format!("/room/{code}/tm/gift-roll"), "slot=0").await;
                }
            }
        }
    }
}

#[tokio::test]
async fn test_tm_roll_any_member_and_autologs() {
    let (app, pool) = test_app_with_pool().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;
    post_form(&app, &alice, &format!("/room/{code}/tm/start"), "").await;
    let alice_id = drinkinggame::db::get_player_by_name(&pool, "alice")
        .await
        .unwrap()
        .id;
    let bob_id = drinkinggame::db::get_player_by_name(&pool, "bob")
        .await
        .unwrap()
        .id;

    let mut call_total: i64 = 0;
    for _ in 0..500 {
        // Measured immediately around the roll under test — resolving a
        // stray double back to Ready (below) can itself auto-log gift-roll
        // drinks, which would otherwise pollute a single before/after taken
        // at the top of the test.
        let before_total =
            tm_drinks(&pool, &code, "alice").await + tm_drinks(&pool, &code, "bob").await;
        // bob taps ROLL even though alice (the room creator) is the current
        // 3 Man/roller — proves the gate is "any member", not "the roller".
        let res = post_form(&app, &bob, &format!("/room/{code}/tm/roll"), "").await;
        assert_eq!(res.status(), StatusCode::NO_CONTENT);
        let st = tm_state(&pool, &code).await;
        if !st.calls.is_empty() {
            call_total = st.calls.iter().map(|c| c.amount as i64).sum();
            let after_total =
                tm_drinks(&pool, &code, "alice").await + tm_drinks(&pool, &code, "bob").await;
            assert_eq!(after_total, before_total + call_total);
            break;
        }
        tm_resolve_to_ready(&app, &pool, &code, &alice, &bob, alice_id, bob_id).await;
    }
    assert!(call_total > 0, "expected a call to fire within 500 rolls");
}

#[tokio::test]
async fn test_tm_roll_wrong_phase_409() {
    let app = test_app().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;
    post_form(&app, &alice, &format!("/room/{code}/tm/start"), "").await;

    let res1 = post_form(&app, &alice, &format!("/room/{code}/tm/roll"), "").await;
    assert_eq!(res1.status(), StatusCode::NO_CONTENT);
    // Ready never returns on its own without a pass, so a second roll is
    // always WrongPhase regardless of what dice came up.
    let res2 = post_form(&app, &alice, &format!("/room/{code}/tm/roll"), "").await;
    assert_eq!(res2.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn test_tm_handoff_gating() {
    let (app, pool) = test_app_with_pool().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;
    post_form(&app, &alice, &format!("/room/{code}/tm/start"), "").await;
    let alice_id = drinkinggame::db::get_player_by_name(&pool, "alice")
        .await
        .unwrap()
        .id;
    let bob_id = drinkinggame::db::get_player_by_name(&pool, "bob")
        .await
        .unwrap()
        .id;
    let game_id = tm_game_id(&pool, &code).await;

    // Rig: alice is roller and 3 Man; a lone 3 hands off with no calls.
    let mut st = ThreeManState::new(vec![alice_id, bob_id], alice_id);
    st.roll(3, 5).unwrap();
    assert_eq!(st.phase, Phase::HandOff);
    drinkinggame::db::set_game_state(&pool, game_id, &st.to_json()).await;

    // bob is not the roller -> 403.
    let res = post_form(
        &app,
        &bob,
        &format!("/room/{code}/tm/three-man"),
        &format!("target={bob_id}"),
    )
    .await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // alice (the roller) -> 204, and the crown actually moves.
    let res2 = post_form(
        &app,
        &alice,
        &format!("/room/{code}/tm/three-man"),
        &format!("target={bob_id}"),
    )
    .await;
    assert_eq!(res2.status(), StatusCode::NO_CONTENT);
    let after = tm_state(&pool, &code).await;
    assert_eq!(after.three_man, bob_id);
}

#[tokio::test]
async fn test_tm_double_owner_gating() {
    let (app, pool) = test_app_with_pool().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;
    post_form(&app, &alice, &format!("/room/{code}/tm/start"), "").await;
    let alice_id = drinkinggame::db::get_player_by_name(&pool, "alice")
        .await
        .unwrap()
        .id;
    let bob_id = drinkinggame::db::get_player_by_name(&pool, "bob")
        .await
        .unwrap()
        .id;
    let game_id = tm_game_id(&pool, &code).await;

    // Rig: alice rolls a double -> she owns the Assign phase.
    let mut st = ThreeManState::new(vec![alice_id, bob_id], alice_id);
    st.roll(4, 4).unwrap();
    assert_eq!(st.phase, Phase::Assign);
    drinkinggame::db::set_game_state(&pool, game_id, &st.to_json()).await;

    // bob isn't the double's owner -> 403.
    let res = post_form(&app, &bob, &format!("/room/{code}/tm/mode"), "mode=both").await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // bob isn't the owner on /tm/target or /tm/clear-slot either — same
    // gate, same owner check, both action routes covered too.
    let res_target = post_form(
        &app,
        &bob,
        &format!("/room/{code}/tm/target"),
        &format!("slot=0&target={bob_id}"),
    )
    .await;
    assert_eq!(res_target.status(), StatusCode::FORBIDDEN);
    let res_clear = post_form(&app, &bob, &format!("/room/{code}/tm/clear-slot"), "slot=0").await;
    assert_eq!(res_clear.status(), StatusCode::FORBIDDEN);

    // alice is the owner -> 204.
    let res2 = post_form(&app, &alice, &format!("/room/{code}/tm/mode"), "mode=both").await;
    assert_eq!(res2.status(), StatusCode::NO_CONTENT);

    // 2 players: split needs a 3rd -> TooFewPlayers (409), exercising the
    // one map_tm arm the rest of this test's 403/204 pairs don't reach.
    let res_split = post_form(&app, &alice, &format!("/room/{code}/tm/mode"), "mode=split").await;
    assert_eq!(res_split.status(), StatusCode::CONFLICT);
    assert!(body_string(res_split).await.contains("at least 2 players"));
}

/// `double` is only cleared at the start of the next `roll()` — not by
/// `pass()` — so a finished double sits around as `Some(stale_owner)` for
/// the entire window between the gift round ending and the next roll. A
/// stranger to that dead double posting to an owner-gated route during that
/// window must get 409 OutOfTurn ("no double running"), not 403
/// NotYourCall (which would wrongly imply a double IS running and they're
/// just not it).
#[tokio::test]
async fn test_tm_stale_double_after_pass_is_out_of_turn() {
    let (app, pool) = test_app_with_pool().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;
    post_form(&app, &alice, &format!("/room/{code}/tm/start"), "").await;
    let alice_id = drinkinggame::db::get_player_by_name(&pool, "alice")
        .await
        .unwrap()
        .id;
    let bob_id = drinkinggame::db::get_player_by_name(&pool, "bob")
        .await
        .unwrap()
        .id;
    let game_id = tm_game_id(&pool, &code).await;

    // Rig: alice's double resolves fully, then she passes -> phase is Ready
    // again but the stale `double` (owner alice) hasn't been cleared yet —
    // that only happens inside the next roll().
    let mut st = ThreeManState::new(vec![alice_id, bob_id], alice_id);
    st.roll(4, 4).unwrap();
    st.set_mode(GiveMode::Both).unwrap();
    st.pick_target(0, bob_id).unwrap();
    st.send().unwrap();
    st.gift_roll(0, vec![1, 2]).unwrap();
    st.pass().unwrap();
    assert_eq!(st.phase, Phase::Ready);
    assert!(st.double.is_some(), "double should still be the stale one");
    drinkinggame::db::set_game_state(&pool, game_id, &st.to_json()).await;

    let res = post_form(&app, &bob, &format!("/room/{code}/tm/send"), "").await;
    assert_eq!(res.status(), StatusCode::CONFLICT); // 409, not 403
}

#[tokio::test]
async fn test_tm_gift_roll_autolog_and_payback() {
    let (app, pool) = test_app_with_pool().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;
    post_form(&app, &alice, &format!("/room/{code}/tm/start"), "").await;
    let alice_id = drinkinggame::db::get_player_by_name(&pool, "alice")
        .await
        .unwrap()
        .id;
    let bob_id = drinkinggame::db::get_player_by_name(&pool, "bob")
        .await
        .unwrap()
        .id;
    let game_id = tm_game_id(&pool, &code).await;

    let mut payback_fired = false;
    for _ in 0..300 {
        // Fresh Gifts phase each attempt: alice owns a double(4), bob is the
        // sole "both dice" victim.
        let mut st = ThreeManState::new(vec![alice_id, bob_id], alice_id);
        st.roll(4, 4).unwrap();
        st.set_mode(GiveMode::Both).unwrap();
        st.pick_target(0, bob_id).unwrap();
        st.send().unwrap();
        assert_eq!(st.phase, Phase::Gifts);
        drinkinggame::db::set_game_state(&pool, game_id, &st.to_json()).await;

        let before_bob = tm_drinks(&pool, &code, "bob").await;
        let before_alice = tm_drinks(&pool, &code, "alice").await;

        // Any member — not just the owner or victim — may roll the gift dice.
        let res = post_form(&app, &bob, &format!("/room/{code}/tm/gift-roll"), "slot=0").await;
        assert_eq!(res.status(), StatusCode::NO_CONTENT);

        let after_st = tm_state(&pool, &code).await;
        let gift = &after_st.double.as_ref().unwrap().gifts[0];
        let total: i64 = gift
            .values
            .as_ref()
            .unwrap()
            .iter()
            .map(|&v| v as i64)
            .sum();

        let after_bob = tm_drinks(&pool, &code, "bob").await;
        assert_eq!(after_bob, before_bob + total, "victim auto-log mismatch");

        if let Some(payback) = after_st.double.as_ref().unwrap().payback {
            let after_alice = tm_drinks(&pool, &code, "alice").await;
            assert_eq!(
                after_alice,
                before_alice + payback as i64,
                "owner payback auto-log mismatch"
            );
            payback_fired = true;
            break;
        }
    }
    assert!(
        payback_fired,
        "expected payback to fire within 300 attempts"
    );
}

#[tokio::test]
async fn test_tm_seat_and_table_reassign_any_member() {
    let (app, pool) = test_app_with_pool().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;
    post_form(&app, &alice, &format!("/room/{code}/tm/start"), "").await;
    let alice_id = drinkinggame::db::get_player_by_name(&pool, "alice")
        .await
        .unwrap()
        .id;
    let bob_id = drinkinggame::db::get_player_by_name(&pool, "bob")
        .await
        .unwrap()
        .id;

    // Fresh game: Ready phase, alice is roller/3 Man, bob is neither — yet
    // bob (any member) can move seats and reassign the 3 Man outside HandOff.
    let res = post_form(
        &app,
        &bob,
        &format!("/room/{code}/tm/seat"),
        &format!("target={alice_id}&dir=1"),
    )
    .await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    let res2 = post_form(
        &app,
        &bob,
        &format!("/room/{code}/tm/three-man"),
        &format!("target={bob_id}"),
    )
    .await;
    assert_eq!(res2.status(), StatusCode::NO_CONTENT);

    let st = tm_state(&pool, &code).await;
    assert_eq!(st.three_man, bob_id);
}

#[tokio::test]
async fn test_tm_pass_after_rolled() {
    let (app, pool) = test_app_with_pool().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;
    post_form(&app, &alice, &format!("/room/{code}/tm/start"), "").await;
    let alice_id = drinkinggame::db::get_player_by_name(&pool, "alice")
        .await
        .unwrap()
        .id;
    let bob_id = drinkinggame::db::get_player_by_name(&pool, "bob")
        .await
        .unwrap()
        .id;
    let game_id = tm_game_id(&pool, &code).await;

    // Rig a plain non-double roll -> deterministic Rolled phase.
    let mut st = ThreeManState::new(vec![alice_id, bob_id], alice_id);
    st.roll(2, 5).unwrap();
    assert_eq!(st.phase, Phase::Rolled);
    drinkinggame::db::set_game_state(&pool, game_id, &st.to_json()).await;

    // bob (any member, not the roller) passes.
    let res = post_form(&app, &bob, &format!("/room/{code}/tm/pass"), "").await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    let after = tm_state(&pool, &code).await;
    assert_eq!(after.roller(), bob_id);
    assert!(after.stale);
}

#[tokio::test]
async fn test_midgame_join_appends_to_order() {
    use futures::StreamExt;
    let (app, pool) = test_app_with_pool().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let carol = login(&app, "carol", "9999").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;
    post_form(&app, &alice, &format!("/room/{code}/tm/start"), "").await;

    let sse_res = app
        .clone()
        .oneshot(
            Request::get(format!("/room/{code}/sse"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let mut sse_body = sse_res.into_body().into_data_stream();
    for _ in 0..4 {
        sse_body.next().await.unwrap().unwrap(); // drain the 4-frame snapshot
    }

    room_page_html(&app, &carol, &code).await; // carol joins mid-game

    let st = tm_state(&pool, &code).await;
    assert_eq!(st.order.len(), 3);
    let carol_id = drinkinggame::db::get_player_by_name(&pool, "carol")
        .await
        .unwrap()
        .id;
    assert!(st.order.contains(&carol_id));

    // broadcast_room fires on every room-page join regardless of game state,
    // so it alone wouldn't prove the mid-game join hook ran — but
    // broadcast_game only fires when the hook actually appended a new
    // player to a running 3 Man's order, so that's the frame that proves it.
    // The handler emits them in that order (room, then game) while still
    // holding the room lock.
    let room_frame = String::from_utf8(sse_body.next().await.unwrap().unwrap().to_vec()).unwrap();
    assert!(room_frame.contains("event: room"));
    let game_frame = String::from_utf8(sse_body.next().await.unwrap().unwrap().to_vec()).unwrap();
    assert!(game_frame.contains("event: game"));
}

#[tokio::test]
async fn test_non_member_tm_403() {
    let app = test_app().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;
    post_form(&app, &alice, &format!("/room/{code}/tm/start"), "").await;

    // carol is logged in (has a valid session) but never joined the room.
    let carol = login(&app, "carol", "9999").await;
    let res = post_form(&app, &carol, &format!("/room/{code}/tm/roll"), "").await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}
