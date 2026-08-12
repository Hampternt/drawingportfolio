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
        ("/assets/lastcall.css", "text/css"),
        ("/assets/htmx.min.js", "application/javascript"),
        ("/assets/lc_motion.js", "application/javascript"),
        ("/assets/lc_wheel.js", "application/javascript"),
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

/// CSS comments don't nest: a `/*` inside a comment is inert, so the comment
/// closes at the first `*/` and the leftover text invalidates the NEXT rule,
/// which the browser silently drops (this broke `.card-big` once — the card
/// face lost its size/background/position and the corner ranks escaped to
/// the page corners).
async fn assert_no_nested_comments(app: &Router, path: &str) {
    let res = app
        .clone()
        .oneshot(Request::get(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let css = body_string(res).await;

    let mut in_comment = false;
    let mut line = 1usize;
    let bytes = css.as_bytes();
    let mut i = 0;
    while i + 1 < bytes.len() {
        match (&bytes[i..i + 2], in_comment) {
            (b"/*", false) => {
                in_comment = true;
                i += 2;
                continue;
            }
            (b"/*", true) => {
                panic!("nested /* inside a CSS comment at line {line} in {path} — the comment closes early and the following rule gets dropped by the browser")
            }
            (b"*/", true) => {
                in_comment = false;
                i += 2;
                continue;
            }
            _ => {}
        }
        if bytes[i] == b'\n' {
            line += 1;
        }
        i += 1;
    }
    assert!(!in_comment, "unterminated CSS comment in {path}");
}

#[tokio::test]
async fn test_game_css_has_no_nested_comment_markers() {
    let app = test_app().await;
    assert_no_nested_comments(&app, "/assets/game.css").await;
}

#[tokio::test]
async fn test_lastcall_css_has_no_nested_comment_markers() {
    let app = test_app().await;
    assert_no_nested_comments(&app, "/assets/lastcall.css").await;
}

#[tokio::test]
async fn test_lastcall_css_has_deck_ramps() {
    let app = test_app().await;
    let css = body_string(get(&app, "/assets/lastcall.css").await).await;
    for needle in [
        ".lc-deck-beer",
        ".lc-deck-cider",
        ".lc-deck-wine",
        ".lc-deck-liquor",
        ".lc-deck-soft",
        "#D4657F",
        "--lc-ink-66: #D4657F66",
        "#0D1620",
    ] {
        assert!(css.contains(needle), "missing {needle}");
    }
}

#[tokio::test]
async fn test_lastcall_css_has_base_reset() {
    let app = test_app().await;
    let css = body_string(get(&app, "/assets/lastcall.css").await).await;
    assert!(css.contains("box-sizing: border-box"));
    assert!(css.contains("appearance: none"));
}

/// findings 1 and 6: CSS geometry/computed-value bugs aren't testable from
/// this suite by rendering — the honest covering test is a sheet assertion
/// pinning the corrected declaration, not a claim that the render was
/// verified in a browser.
#[tokio::test]
async fn test_lastcall_css_pins_deckless_back_and_deckstack_shadow_fixes() {
    let app = test_app().await;
    let css = body_string(get(&app, "/assets/lastcall.css").await).await;
    // finding 1: .lc-back's border must fall back when no ancestor
    // .lc-deck-* supplies --lc-ink-80 (DiscardSlot's back is deliberately
    // deckless) — without the fallback, the border shorthand is invalid at
    // computed-value time and renders 3px dashed currentColor instead of a
    // 1px hairline.
    assert!(
        css.contains("border: 1px solid var(--lc-ink-80, var(--lc-hair-strong))"),
        "deckless .lc-back border has no fallback for --lc-ink-80"
    );
    // finding 6: .lc-back needs its own positioned containing block inside
    // .lc-deckstack, or the offset-shadow ::before's `inset: 0` resolves
    // against the whole (unpositioned) .lc-back falls through to
    // .lc-deckstack, painting a full-column slab instead of a 68x92 shadow
    // card.
    assert!(
        css.contains(".lc-deckstack .lc-back { position: relative; }"),
        "DeckStack's card back is not a positioned containing block for its offset-shadow ::before"
    );
    // finding 7: the plaque's 3px deck rule must sit flush at the top
    // (border-top-equivalent), not floating inset inside the padding box —
    // padding-top: 0 on .lc-plaque plus negative side margins on its .lc-rule
    // child pull the rule to the plaque's very edges.
    assert!(
        css.contains("padding: 0 14px 12px; border-radius: 10px; overflow: hidden;"),
        ".lc-plaque no longer has padding-top: 0 (the deck rule would float inset again)"
    );
    assert!(
        css.contains(".lc-plaque > .lc-rule { margin: 0 -14px; }"),
        "the plaque's deck rule is missing its flush-to-edge negative margins"
    );
}

#[tokio::test]
async fn test_lastcall_css_has_every_component_root() {
    let app = test_app().await;
    let css = body_string(get(&app, "/assets/lastcall.css").await).await;
    // Roots/classes assembled from Task 2's Produces set (task-2-brief.md) —
    // not from what the sheet happens to contain — each must carry an
    // actual CSS rule in this slice.
    for needle in [
        ".lc-cardface",
        ".lc-pip",
        ".lc-mini",
        ".lc-back",
        ".lc-dot",
        ".lc-plaque",
        ".lc-handstrip",
        ".lc-deckstack",
        ".lc-discard",
        ".lc-face-kws",
        "#lc-banner",
        "#lc-felt",
        "#lc-flights",
        "#lc-hand",
    ] {
        assert!(css.contains(needle), "missing {needle}");
    }
    // Also in Task 2's Produces set, but a deliberate Plan A-vis deferral
    // (motion/animation), not a miss — kept in a separate array so this test
    // still fails on a real, undocumented miss instead of passing on the
    // presence of the deferral comment's own token text. Each entry must be
    // named AND its deferral documented, so the next reader doesn't file it
    // as a bug and a later slice doesn't silently drop the comment.
    let (needle, deferral_marker) = ("#lc-beat-timer", "Plan A-vis");
    assert!(css.contains(needle), "missing {needle}");
    assert!(
        css.contains(deferral_marker),
        "{needle} present but no comment documents its Plan A-vis deferral"
    );
}

#[tokio::test]
async fn test_lastcall_css_has_every_keyframe() {
    let app = test_app().await;
    let css = body_string(get(&app, "/assets/lastcall.css").await).await;
    for needle in [
        "@keyframes lc-fly",
        "@keyframes lc-dot",
        "@keyframes lc-shake",
        "@keyframes lc-hp-flash",
        "@keyframes lc-pulse",
        "@keyframes lc-banner",
        "@keyframes lc-timer",
    ] {
        assert!(css.contains(needle), "missing {needle}");
    }
}

#[tokio::test]
async fn test_lastcall_css_reduced_motion_is_one_block() {
    let app = test_app().await;
    let css = body_string(get(&app, "/assets/lastcall.css").await).await;
    let marker = "prefers-reduced-motion: reduce";
    let count = css.matches(marker).count();
    assert_eq!(
        count, 1,
        "expected exactly one {marker} block, found {count}"
    );

    // Walk from the marker to the matching close of the @media block itself
    // (brace-depth 0), not just the first inner rule's `}` — the assertions
    // below must see the whole block, including rules past the first one.
    let start = css.find(marker).expect("marker present");
    let open = css[start..]
        .find('{')
        .map(|i| start + i)
        .expect("block opens");
    let mut depth = 0i32;
    let mut end = css.len();
    for (i, ch) in css[open..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    end = open + i + 1;
                    break;
                }
            }
            _ => {}
        }
    }
    let block = &css[start..end];
    assert!(block.contains(".lc-flight"), "block missing .lc-flight");
    assert!(
        block.contains("animation: none"),
        "block missing animation: none"
    );
}

/// Shared by `test_lastcall_css_preview_body_is_positioned` and
/// `test_lastcall_css_shell_body_is_positioned` (plan-end review finding M4):
/// `#lc-flights` is `position: absolute; inset: 0; overflow: hidden` (Plan
/// A). Without a positioned ancestor, that layer resolves `inset: 0` against
/// the viewport-sized initial containing block instead of the full document,
/// and every flight is created, positioned and animated correctly, then
/// clipped away the moment it scrolls past the first screenful — a silent,
/// un-renderable failure no other test can see, because the DOM and the CSS
/// are each correct in isolation; only the composition is wrong. This can't
/// be asserted by rendering (this suite has no browser), so the honest
/// covering test is at the CSS level: `selector` must carry a `position`
/// that is not `static`. If a future edit deletes the rule as "unused" —
/// nothing on the page visibly depends on it yet — this is what catches it.
fn assert_body_selector_is_positioned(css: &str, selector: &str, finding: &str) {
    // Anchored on the literal selector-then-brace text, not just the bare
    // selector — `body.lc` is also a substring of `body.lc-preview`, and
    // both selectors appear inside this file's own explanatory comments.
    let needle = format!("{selector} {{");
    let start = css
        .find(&needle)
        .unwrap_or_else(|| panic!("missing `{needle}` rule"));
    let open = start + needle.len() - 1; // index of the rule's own '{'
    let close = css[open..]
        .find('}')
        .map(|i| open + i)
        .unwrap_or_else(|| panic!("{selector} block closes"));
    let block = &css[open..close];

    assert!(
        !block.contains("position: static") && block.contains("position:"),
        "{selector} must declare a non-static position — #lc-flights (Plan \
         A: position: absolute; inset: 0; overflow: hidden) has no other \
         positioned ancestor and needs this rule for its containing block, \
         or every flight renders off-document and invisible ({finding}). \
         Block was: {block:?}"
    );
}

/// Plan-end review finding C1: see `assert_body_selector_is_positioned` for
/// the mechanism. `body.lc-preview` is the gallery page's own containing
/// block for `#lc-flights`.
#[tokio::test]
async fn test_lastcall_css_preview_body_is_positioned() {
    let app = test_app().await;
    let css = body_string(get(&app, "/assets/lastcall.css").await).await;
    assert_body_selector_is_positioned(&css, "body.lc-preview", "plan-end review finding C1");
}

/// Plan-end review finding M4: `body.lc` (the F.1 phone shell) carries the
/// identical `position: relative` rule for the identical reason — nothing
/// fires a flight on the shell yet, so nothing visibly depends on it either,
/// which is exactly why `body.lc-preview`'s test alone wasn't enough: this
/// rule could be deleted as "unused" and every other test would stay green
/// until a later slice starts firing flights into `#lc-flights` on the
/// shell and finds them silently clipped away.
#[tokio::test]
async fn test_lastcall_css_shell_body_is_positioned() {
    let app = test_app().await;
    let css = body_string(get(&app, "/assets/lastcall.css").await).await;
    assert_body_selector_is_positioned(&css, "body.lc", "plan-end review finding M4");
}

/// Plan-end review finding I3, human ruling: the plan's Global Constraints
/// and Task 3 Step 1 both specify a four-rung deck-tinted border alpha
/// ladder (59/66/80/99); Plan A's stylesheet originally bound only three.
/// Pins all four bound utility classes and the --lc-ink-99 token itself for
/// every deck, so the ladder cannot silently regress back to three.
#[tokio::test]
async fn test_lastcall_css_ink_alpha_ladder_has_four_rungs() {
    let app = test_app().await;
    let css = body_string(get(&app, "/assets/lastcall.css").await).await;

    for class_rule in [
        ".lc-edge-subtle { border: 1px solid var(--lc-ink-59); }",
        ".lc-edge-plaque { border: 1px solid var(--lc-ink-66); }",
        ".lc-edge-back   { border: 1px solid var(--lc-ink-80); }",
        ".lc-edge-strong { border: 1px solid var(--lc-ink-99); }",
    ] {
        assert!(
            css.contains(class_rule),
            "missing bound alpha rung: {class_rule}"
        );
    }

    // The --lc-ink-NN token itself, for every one of the five decks. Wine's
    // ink hue (#D4657F) differs from its fill (#8B2F4A); the other four
    // decks' ink equals their fill.
    for (slug, ink_hex) in [
        ("beer", "FFB570"),
        ("cider", "B48EF7"),
        ("wine", "D4657F"),
        ("liquor", "F7768E"),
        ("soft", "6FB6FF"),
    ] {
        for alpha in ["59", "66", "80", "99"] {
            let needle = format!("--lc-ink-{alpha}: #{ink_hex}{alpha}");
            assert!(css.contains(&needle), "deck {slug} missing {needle}");
        }
    }
}

#[tokio::test]
async fn test_lc_motion_js_binds_both_lifecycle_events() {
    let app = test_app().await;
    let js = body_string(get(&app, "/assets/lc_motion.js").await).await;
    for needle in [
        "DOMContentLoaded",
        "htmx:afterSwap",
        "animationend",
        "data-flight-anchor",
    ] {
        assert!(js.contains(needle), "missing {needle}");
    }
}

#[tokio::test]
async fn test_lc_wheel_js_is_served_and_binds_both_lifecycle_events() {
    let app = test_app().await;
    let res = get(&app, "/assets/lc_wheel.js").await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        res.headers()[header::CONTENT_TYPE],
        "application/javascript"
    );
    let js = body_string(res).await;
    assert!(!js.is_empty());
    for needle in ["DOMContentLoaded", "htmx:afterSwap", "lcWheelInit"] {
        assert!(js.contains(needle), "missing {needle}");
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

/// `/tm/end` must re-broadcast the standings, not just the game/screen/room
/// surfaces: the "3 MAN" badge on the outgoing 3 Man's leaderboard row is
/// only ever cleared by a fresh `leaderboard` render, and `render_leaderboard`
/// is kind-aware (`db::get_active_game` returns `None` once `end_game` has
/// run) — so a broadcast here is enough to drop it without any special-casing
/// in this handler. Without it, the badge would linger until the next
/// unrelated drink/undo happened to trigger one.
#[tokio::test]
async fn test_tm_end_broadcasts_leaderboard_without_badge() {
    use futures::StreamExt;
    let app = test_app().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;
    // alice starts the game, so `three_man == alice` — her row carries the
    // badge from the moment the game begins.
    post_form(&app, &alice, &format!("/room/{code}/tm/start"), "").await;

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
    // Drain the 4-frame snapshot (leaderboard/game/screen/room), which
    // carries the badge since the game is still active at connect time.
    let mut snapshot_leaderboard_has_badge = false;
    for _ in 0..4 {
        let frame = String::from_utf8(body.next().await.unwrap().unwrap().to_vec()).unwrap();
        if frame.contains("event: leaderboard") {
            snapshot_leaderboard_has_badge = frame.contains("tm-chip");
        }
    }
    assert!(
        snapshot_leaderboard_has_badge,
        "sanity check: the connect-time snapshot should carry the badge while the game is active"
    );

    let res = post_form(&app, &alice, &format!("/room/{code}/tm/end"), "").await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    // game, screen, room are the three known frames tm/end already sent
    // (test_tm_end_broadcasts_summary_and_idle covers their content) — drain
    // them here just to reach the leaderboard frame this test targets.
    for _ in 0..3 {
        body.next().await.unwrap().unwrap();
    }

    let leaderboard_frame = tokio::time::timeout(std::time::Duration::from_secs(2), body.next())
        .await
        .expect("tm/end should broadcast a leaderboard refresh once the game ends")
        .unwrap()
        .unwrap();
    let leaderboard_frame = String::from_utf8(leaderboard_frame.to_vec()).unwrap();
    assert!(leaderboard_frame.contains("event: leaderboard"));
    assert!(
        !leaderboard_frame.contains("tm-chip") && !leaderboard_frame.contains("3 MAN"),
        "leaderboard frame after tm/end should have dropped the 3 MAN badge: {leaderboard_frame:?}"
    );
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

// -------------------------------------------------------------
// Task 14: Phase-2 integration polish — cross-kind + snapshot + login
// round-trip coverage closing the spec's Testing checklist gaps left after
// Tasks 12-13.
// -------------------------------------------------------------

/// SSE connect during a running 3 Man game: the `game` snapshot is the
/// kind-dispatched seat-strip panel (carries `data-order` for the client's
/// seat rendering), and the `room` snapshot carries the 3 MAN topbar chip
/// (`room_panel`'s `data-mode == "three_man"` branch) — not the Ring of
/// Fire idle/active panels the shared snapshot code defaults to.
#[tokio::test]
async fn test_tm_sse_snapshot_includes_tm_panels() {
    use futures::StreamExt;
    let (app, pool) = test_app_with_pool().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await; // bob joins
    post_form(&app, &alice, &format!("/room/{code}/tm/start"), "").await;
    let alice_id = drinkinggame::db::get_player_by_name(&pool, "alice")
        .await
        .unwrap()
        .id;
    let bob_id = drinkinggame::db::get_player_by_name(&pool, "bob")
        .await
        .unwrap()
        .id;

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

    let leaderboard_frame =
        String::from_utf8(body.next().await.unwrap().unwrap().to_vec()).unwrap();
    assert!(leaderboard_frame.contains("event: leaderboard"));

    let game_frame = String::from_utf8(body.next().await.unwrap().unwrap().to_vec()).unwrap();
    assert!(game_frame.contains("event: game"));
    assert!(
        game_frame.contains(&format!(r#"data-order="{alice_id},{bob_id}""#)),
        "3 Man game snapshot should carry the seat strip's data-order for the seeded rotation \
         (alice created the room -> she's seat 0): {game_frame:?}"
    );

    let screen_frame = String::from_utf8(body.next().await.unwrap().unwrap().to_vec()).unwrap();
    assert!(screen_frame.contains("event: screen"));

    let room_frame = String::from_utf8(body.next().await.unwrap().unwrap().to_vec()).unwrap();
    assert!(room_frame.contains("event: room"));
    assert!(
        room_frame.contains(r#"<span class="tm-chip">3 MAN</span>"#),
        "3 Man room snapshot should carry the 3 MAN chip: {room_frame:?}"
    );
}

/// The initial (non-SSE) render of both `/room/{code}` and `/room/{code}/screen`
/// must carry the same 3 MAN standings badge the SSE snapshot does — both
/// build their leaderboard HTML from a `rows` query directly rather than
/// going through the kind-aware `render_leaderboard` helper the SSE path
/// uses, so a first paint during a running 3 Man game would otherwise flash
/// badge-less until the next leaderboard broadcast.
#[tokio::test]
async fn test_room_and_screen_pages_render_tm_badge_on_initial_load() {
    let app = test_app().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;
    // alice starts the game, so `three_man == alice` — her row carries the
    // badge from the moment the game begins.
    post_form(&app, &alice, &format!("/room/{code}/tm/start"), "").await;

    let expected_badge_row =
        r#"<span class="lb-name">alice</span><span class="tm-chip">3 MAN</span>"#;

    let room_html = room_page_html(&app, &alice, &code).await;
    assert!(
        room_html.contains(expected_badge_row),
        "room page's initial leaderboard render should carry the 3 MAN badge: {room_html:?}"
    );

    let res = app
        .oneshot(
            Request::get(format!("/room/{code}/screen"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let screen_html = body_string(res).await;
    assert!(
        screen_html.contains(expected_badge_row),
        "screen page's initial leaderboard render should carry the 3 MAN badge: {screen_html:?}"
    );
}

/// Regression coverage for the redesign that added kind-dispatch to
/// `current_panel`/`current_screen_panel` (three_man vs ring_of_fire): the
/// 52nd draw must still broadcast the final card BEFORE the game-over
/// summary, exactly as Task 11 established, now that the shared dispatch
/// point routes through a kind match before falling into the RoF path.
/// Genuinely cross-kind (not just a rename of Task 11's test): a full 3 Man
/// game is played to completion in the same room FIRST, so the room's
/// `games` table already holds an ended `three_man` row — and
/// `get_active_game`'s `ended_at IS NULL` filter, which every dispatch point
/// relies on to pick the right kind, is exercised against real history
/// rather than an empty table.
#[tokio::test]
async fn test_rof_full_deck_still_ends_after_redesign() {
    use futures::StreamExt;
    let app = test_app().await;
    let cookie = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let code = create_room(&app, &cookie).await;
    room_page_html(&app, &bob, &code).await; // bob joins — 3 Man needs 2+

    // Play a 3 Man game to completion first, leaving an ended `three_man`
    // row behind in this room's `games` table.
    post_form(&app, &cookie, &format!("/room/{code}/tm/start"), "").await;
    let res = post_form(&app, &cookie, &format!("/room/{code}/tm/end"), "").await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

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
        body.next().await.unwrap().unwrap(); // drain the initial snapshot
    }

    let res = post_form(&app, &cookie, &format!("/room/{code}/game/draw"), "").await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    // Final (52nd) card broadcasts first — still an active panel, not the
    // game-over summary.
    let first = String::from_utf8(body.next().await.unwrap().unwrap().to_vec()).unwrap();
    assert!(first.contains("event: game"));
    assert!(first.contains("0 LEFT"));
    assert!(!first.contains("GAME OVER"));

    // Screen mirrors the same final card.
    let screen_active = String::from_utf8(body.next().await.unwrap().unwrap().to_vec()).unwrap();
    assert!(screen_active.contains("event: screen"));
    assert!(screen_active.contains("0 of 52 left"));

    // Only THEN does the game-over summary follow.
    let summary = String::from_utf8(body.next().await.unwrap().unwrap().to_vec()).unwrap();
    assert!(summary.contains("event: game"));
    assert!(summary.contains("GAME OVER"));

    // Room settles back to idle.
    assert!(room_page_html(&app, &cookie, &code)
        .await
        .contains(">START<"));
}

/// Documents the accepted caveat: gift auto-log calls `insert_events_bulk`
/// (one `events` row per drink), while `/undo` only tombstones the caller's
/// single most-recent row — so a gift of 3 followed by exactly one undo
/// nets 2, not 0. Drives a real Both-mode gift through the actual
/// `/tm/gift-roll` route (real dice roll, real auto-log) each attempt,
/// retrying only the setup until the roll happens to total 3 — no unbounded
/// loop, bounded at 500 attempts with a probability of ~2/36 per attempt
/// (P(never hits) < 1e-12), and the assertion itself always runs exactly
/// once against a real roll.
#[tokio::test]
async fn test_undo_after_gift_is_per_row() {
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

    let mut hit = false;
    for _ in 0..500 {
        // Fresh Gifts phase each attempt: alice owns a double(4) sent Both
        // to bob (dice_count 2 -> total 2..=12, exactly 3 only via {1,2}).
        let mut st = ThreeManState::new(vec![alice_id, bob_id], alice_id);
        st.roll(4, 4).unwrap();
        st.set_mode(GiveMode::Both).unwrap();
        st.pick_target(0, bob_id).unwrap();
        st.send().unwrap();
        drinkinggame::db::set_game_state(&pool, game_id, &st.to_json()).await;

        let before = tm_drinks(&pool, &code, "bob").await;
        let res = post_form(&app, &bob, &format!("/room/{code}/tm/gift-roll"), "slot=0").await;
        assert_eq!(res.status(), StatusCode::NO_CONTENT);
        let after_gift = tm_drinks(&pool, &code, "bob").await;
        if after_gift != before + 3 {
            continue;
        }

        let res = post_form(&app, &bob, &format!("/room/{code}/undo"), "").await;
        assert_eq!(res.status(), StatusCode::NO_CONTENT);
        let after_undo = tm_drinks(&pool, &code, "bob").await;
        assert_eq!(
            after_undo,
            before + 2,
            "one undo should tombstone exactly one of the gift's 3 rows, not all of them"
        );
        hit = true;
        break;
    }
    assert!(
        hit,
        "expected a Both-mode gift to total exactly 3 within 500 attempts"
    );
}

/// Ending the night mid-3-Man must leave no `games` row with
/// `ended_at IS NULL` — matching the guarantee `test_end_night_ends_game_and_room`
/// already established for Ring of Fire.
#[tokio::test]
async fn test_ending_room_with_tm_game_no_orphan() {
    let (app, pool) = test_app_with_pool().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;
    post_form(&app, &alice, &format!("/room/{code}/tm/start"), "").await;
    let game_id = tm_game_id(&pool, &code).await;

    let res = post_form(&app, &alice, &format!("/room/{code}/end"), "").await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    let ended: (Option<String>,) = sqlx::query_as("SELECT ended_at FROM games WHERE id = ?1")
        .bind(game_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(
        ended.0.is_some(),
        "3 Man game row must be ended when the room is ended"
    );

    let orphans: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM games WHERE ended_at IS NULL")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        orphans.0, 0,
        "ending the room mid-3-Man must leave no dangling active game rows"
    );
}

// --- account: rename, PIN change, logout -----------------------------------

#[tokio::test]
async fn test_account_page_requires_session() {
    let app = test_app().await;
    let res = get(&app, "/account").await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(res.headers()[header::LOCATION], "/");
}

#[tokio::test]
async fn test_account_page_shows_name_and_forms() {
    let app = test_app().await;
    let cookie = login(&app, "hampter", "1234").await;
    let res = app
        .oneshot(
            Request::get("/account")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let html = body_string(res).await;
    assert!(html.contains("hampter"));
    assert!(html.contains(r#"action="/account/name""#));
    assert!(html.contains(r#"action="/account/pin""#));
    assert!(html.contains(r#"action="/logout""#));
}

/// Logout must kill the session row, not just the cookie — otherwise the id
/// stays valid for its full 90 days on anything that kept a copy.
#[tokio::test]
async fn test_logout_clears_cookie_and_deletes_session() {
    let (app, pool) = test_app_with_pool().await;
    let cookie = login(&app, "hampter", "1234").await;

    let res = post_form(&app, &cookie, "/logout", "").await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let set = res.headers()[header::SET_COOKIE].to_str().unwrap();
    assert!(set.contains("Max-Age=0"), "{set}");
    // Path must match session_cookie's or the browser ignores the clear.
    assert!(set.contains("Path=/"), "{set}");

    let sessions: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM sessions")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(sessions.0, 0);

    // The old cookie value is now worthless server-side.
    let res = app
        .oneshot(
            Request::get("/account")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(res.headers()[header::LOCATION], "/");
}

#[tokio::test]
async fn test_rename_keeps_history_and_frees_the_old_name() {
    let app = test_app().await;
    let cookie = login(&app, "hampter", "1234").await;
    let code = create_room(&app, &cookie).await; // one night on the record

    let res = post_form(&app, &cookie, "/account/name", "name=hampternt").await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(res.headers()[header::LOCATION], "/account");

    // Same player, same session, new name everywhere it is rendered.
    let html = room_page_html(&app, &cookie, &code).await;
    assert!(html.contains("hampternt"));
    assert!(!html.contains(">hampter<"));

    // The vacated name is registerable again, and lands on a *fresh* player:
    // no nights, while the renamed original still owns the one it played.
    let other = login(&app, "hampter", "9999").await;
    let res = app
        .clone()
        .oneshot(
            Request::get("/")
                .header(header::COOKIE, &other)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(body_string(res).await.contains("0 nights"));
    let res = app
        .oneshot(
            Request::get("/")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(body_string(res).await.contains("1 nights"));
}

#[tokio::test]
async fn test_rename_to_someone_elses_name_conflicts() {
    let app = test_app().await;
    login(&app, "alice", "1111").await;
    let cookie = login(&app, "bob", "2222").await;

    let res = post_form(&app, &cookie, "/account/name", "name=alice").await;
    assert_eq!(res.status(), StatusCode::CONFLICT);
    // NOCASE unique index: a different case is still the same name.
    let res = post_form(&app, &cookie, "/account/name", "name=ALICE").await;
    assert_eq!(res.status(), StatusCode::CONFLICT);

    // Bob keeps his name; his login still works.
    let again = login(&app, "bob", "2222").await;
    assert!(!again.is_empty());
}

/// Recasing your own name hits the same UNIQUE index but on your own row,
/// so it must succeed rather than read as a collision.
#[tokio::test]
async fn test_rename_own_name_case_only() {
    let app = test_app().await;
    let cookie = login(&app, "bob", "2222").await;
    let res = post_form(&app, &cookie, "/account/name", "name=Bob").await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
}

#[tokio::test]
async fn test_rename_rejects_blank_name() {
    let app = test_app().await;
    let cookie = login(&app, "bob", "2222").await;
    let res = post_form(&app, &cookie, "/account/name", "name=%20%20").await;
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_pin_change_requires_the_current_pin() {
    let app = test_app().await;
    let cookie = login(&app, "hampter", "1234").await;

    let res = post_form(
        &app,
        &cookie,
        "/account/pin",
        "current_pin=9999&new_pin=4321",
    )
    .await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    // Old PIN still works — nothing was written.
    login(&app, "hampter", "1234").await;
}

#[tokio::test]
async fn test_pin_change_then_login_with_new_pin() {
    let app = test_app().await;
    let cookie = login(&app, "hampter", "1234").await;

    let res = post_form(
        &app,
        &cookie,
        "/account/pin",
        "current_pin=1234&new_pin=4321",
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    login(&app, "hampter", "4321").await;

    // The old PIN is dead.
    let res = post_form(&app, "", "/login", "name=hampter&pin=1234").await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_pin_change_rejects_malformed_new_pin() {
    let app = test_app().await;
    let cookie = login(&app, "hampter", "1234").await;
    let res = post_form(&app, &cookie, "/account/pin", "current_pin=1234&new_pin=12").await;
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
    login(&app, "hampter", "1234").await;
}

/// Both entry points into the account page must exist, or the feature is
/// only reachable by hand-editing the URL.
#[tokio::test]
async fn test_account_link_on_landing_and_in_room_panel() {
    let app = test_app_with_base("/drinks").await;
    let cookie = login(&app, "hampter", "1234").await;
    let res = app
        .clone()
        .oneshot(
            Request::get("/")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(body_string(res).await.contains(r#"href="/drinks/account""#));

    let code = create_room(&app, &cookie).await;
    let html = room_page_html(&app, &cookie, &code).await;
    assert!(html.contains(r#"href="/drinks/account""#));
}

/// Reads body chunks until `marker` has been seen, then returns everything
/// read. Body chunks are NOT one-per-SSE-event — a big panel splits across
/// several — so any test that cares about which events arrived has to read
/// by content, never by counting `next()` calls.
/// Timed out, not open-ended: `KeepAlive` emits a comment chunk every 15s
/// forever, so a regression that drops an expected broadcast would otherwise
/// hang the suite instead of failing it.
async fn read_sse_until(body: &mut axum::body::BodyDataStream, marker: &str) -> String {
    use futures::StreamExt;
    let read = async {
        let mut seen = String::new();
        while !seen.contains(marker) {
            seen.push_str(std::str::from_utf8(&body.next().await.unwrap().unwrap()).unwrap());
        }
        seen
    };
    tokio::time::timeout(std::time::Duration::from_secs(5), read)
        .await
        .unwrap_or_else(|_| panic!("no `{marker}` frame arrived"))
}

/// The rename rebroadcast is the novel behaviour here: names live inside
/// already-swapped SSE fragments, so a rename mid-game has to repush the
/// GAME panel too or the draw log keeps the old name.
#[tokio::test]
async fn test_rename_rebroadcasts_the_game_panel() {
    let (app, pool) = test_app_with_pool().await;
    let alice = login(&app, "alice", "1234").await;
    let code = create_room(&app, &alice).await;
    start_rigged_game_with_rank(&pool, &code, 5).await;
    post_form(&app, &alice, &format!("/room/{code}/game/draw"), "").await;

    let sse_res = get(&app, &format!("/room/{code}/sse")).await;
    let mut sse_body = sse_res.into_body().into_data_stream();
    // `room` is the last of the snapshot's four events.
    read_sse_until(&mut sse_body, "event: room").await;

    post_form(&app, &alice, "/account/name", "name=alicia").await;

    // `screen` is the last of the rename's four.
    let seen = read_sse_until(&mut sse_body, "event: screen").await;
    assert!(seen.contains("event: game"), "{seen}");
    assert!(seen.contains("alicia"), "{seen}");
    assert!(!seen.contains(">alice<"), "{seen}");
}

/// ...but with no game running, `broadcast_game` would publish the *idle*
/// panel — which holds no names at all and would wipe a game-over summary
/// still up on every phone and on the big screen.
#[tokio::test]
async fn test_rename_after_game_over_does_not_clobber_the_summary() {
    let (app, pool) = test_app_with_pool().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;
    post_form(&app, &alice, &format!("/room/{code}/tm/start"), "").await;
    post_form(&app, &alice, &format!("/room/{code}/event"), "kind=drink").await;
    post_form(&app, &alice, &format!("/room/{code}/tm/end"), "").await;

    let room = drinkinggame::db::get_open_room(&pool, &code).await.unwrap();
    assert!(drinkinggame::db::get_active_game(&pool, room.id)
        .await
        .is_none());

    // Subscribe only once the night's game is over, so the wire is quiet and
    // everything after the snapshot is attributable to the rename.
    let sse_res = get(&app, &format!("/room/{code}/sse")).await;
    let mut sse_body = sse_res.into_body().into_data_stream();
    read_sse_until(&mut sse_body, "event: room").await;

    post_form(&app, &alice, "/account/name", "name=alicia").await;
    // Marker: a drink always ends with an `emote` frame, so reading up to it
    // captures exactly what the rename published — and it terminates whether
    // or not the gate is in place.
    post_form(&app, &alice, &format!("/room/{code}/event"), "kind=drink").await;

    let seen = read_sse_until(&mut sse_body, "event: emote").await;
    // Standings and the ROOM tab refresh...
    assert!(seen.contains("event: leaderboard"), "{seen}");
    assert!(seen.contains("event: room"), "{seen}");
    assert!(seen.contains("alicia"), "{seen}");
    // ...but nothing repaints GAME or SCREEN, so the summary survives.
    assert!(!seen.contains("event: game"), "{seen}");
    assert!(!seen.contains("event: screen"), "{seen}");
}

// ---------------------------------------------------------------------
// Last Call — Plan A-vis Task 2: GET /lastcall/preview
// ---------------------------------------------------------------------

/// finding: `boundary_cards()` exists precisely because the catalog cannot
/// sit exactly on the §7.5 thresholds — a mis-typed fixture that quietly
/// lands on the wrong side of a threshold makes the whole boundary group
/// meaningless, so the fixtures themselves are pinned here rather than
/// trusted.
#[test]
fn test_boundary_cards_hit_their_boundaries() {
    let cards: std::collections::HashMap<&str, drinkinggame::last_call::Card> =
        drinkinggame::lc_preview::boundary_cards()
            .into_iter()
            .collect();

    assert_eq!(cards["Title — 14 chars"].title.chars().count(), 14);
    assert_eq!(cards["Title — 15 chars"].title.chars().count(), 15);
    assert_eq!(cards["Title — 24 chars"].title.chars().count(), 24);
    assert_eq!(cards["Title — 25 chars"].title.chars().count(), 25);

    assert_eq!(cards["Body — 108 chars"].text.chars().count(), 108);
    assert_eq!(cards["Body — 109 chars"].text.chars().count(), 109);

    assert_eq!(cards["Keywords — 0"].keywords.len(), 0);
    assert_eq!(cards["Keywords — 3"].keywords.len(), 3);
    assert_eq!(cards["Keywords — 6"].keywords.len(), 6);
}

#[tokio::test]
async fn test_preview_page_is_public() {
    // No login anywhere in this test — that absence is itself the assertion
    // that the route is unguarded, unlike its /presets neighbours.
    let app = test_app_with_base("/drinks").await;
    let res = get(&app, "/lastcall/preview").await;
    assert_eq!(res.status(), StatusCode::OK);
    let html = body_string(res).await;
    assert!(html.contains(r#"href="/drinks/assets/lastcall.css""#));
    assert!(html.contains(r#"src="/drinks/assets/lc_motion.js""#));
    assert!(html.contains(r#"src="/drinks/assets/lc_wheel.js""#));
}

#[tokio::test]
async fn test_preview_renders_five_primitives_in_five_decks() {
    let app = test_app().await;
    let html = body_string(get(&app, "/lastcall/preview").await).await;

    for slug in ["beer", "cider", "wine", "liquor", "soft"] {
        assert!(html.contains(&format!("lc-deck-{slug}")), "missing {slug}");
    }
    for needle in ["lc-cardface", "lc-pip", "lc-mini", "lc-back", "lc-dot"] {
        assert!(html.contains(needle), "missing {needle}");
    }
    assert!(
        html.matches("lc-cardface").count() >= 5,
        "expected at least 5 lc-cardface, got {}",
        html.matches("lc-cardface").count()
    );
    for size in ["strip", "flight", "pile", "stack"] {
        assert!(
            html.contains(&format!(r#"data-size="{size}""#)),
            "missing data-size=\"{size}\""
        );
    }
}

#[tokio::test]
async fn test_preview_shows_every_title_ramp_step() {
    let app = test_app().await;
    let html = body_string(get(&app, "/lastcall/preview").await).await;
    // Asserted on the title element itself (`lc_render::face` emits
    // `class="lc-face-title {ramp}"`), not the swatch caption — the caption
    // is built as `format!("{} — {ramp}", card.title)` and also contains
    // these literal strings, so `html.contains("lc-title-lg")` alone would
    // pass even if card_face emitted no ramp class at all (plan-end review
    // finding I2).
    assert!(html.contains("lc-face-title lc-title-lg"));
    assert!(html.contains("lc-face-title lc-title-md"));
    assert!(html.contains("lc-face-title lc-title-sm"));
}

#[tokio::test]
async fn test_preview_shows_truncation_and_expansion() {
    let app = test_app().await;
    let html = body_string(get(&app, "/lastcall/preview").await).await;
    assert!(html.contains("data-expandable"));
    assert!(html.contains("lc-kw-more"));
    assert!(html.contains("+3"));
    assert!(html.contains("lc-cardface-expanded"));
}

#[tokio::test]
async fn test_preview_shows_every_cost_pip() {
    let app = test_app().await;
    let html = body_string(get(&app, "/lastcall/preview").await).await;
    for cost in ["1", "2", "3"] {
        let needle = format!(r#"data-cost="{cost}""#);
        let count = html.matches(&needle).count();
        assert!(count >= 5, "expected >= 5 {needle}, got {count}");
    }
}

#[tokio::test]
async fn test_preview_has_no_style_element_and_no_behaviour() {
    let app = test_app().await;
    let html = body_string(get(&app, "/lastcall/preview").await).await;
    assert!(!html.contains("<style"));
    for banned in ["hx-post", "hx-get", "onclick"] {
        assert!(!html.contains(banned), "found forbidden `{banned}`");
    }
    // Inline style="--dx:…" custom-property attributes are expected on Task
    // 3's at-rest flight sample; a `style=` attribute containing a literal
    // `#` (a hex colour) is not — colour comes from the stylesheet, only
    // positions/durations may come from custom properties.
    for (i, _) in html.match_indices("style=\"") {
        let rest = &html[i..];
        let end = rest[7..].find('"').map(|e| e + 7).unwrap_or(rest.len());
        let value = &rest[..end];
        assert!(
            !value.contains('#'),
            "style attribute contains a hex colour: {value}"
        );
    }
}

// ---------------------------------------------------------------------
// Last Call — Plan A-vis Task 3: scene, components, states, flights
// ---------------------------------------------------------------------

/// The §7.8.1 test. Every `data-flight-anchor` name must resolve on the
/// preview page — the only place in the series the whole set is provable at
/// once. Markup without an anchor means slice 3 rewrites a template.
#[tokio::test]
async fn test_preview_resolves_every_motion_anchor() {
    let app = test_app().await;
    let html = body_string(get(&app, "/lastcall/preview").await).await;

    let names = [
        "deck-beer",
        "deck-cider",
        "deck-wine",
        "deck-liquor",
        "deck-soft",
        "discard",
        "plaque-seat-0",
        "plaque-seat-1",
        "plaque-seat-2",
        "plaque-seat-3",
        "plaque-seat-4",
        "plaque-seat-5",
        "plaque-seat-6",
        "plaque-seat-7",
        "hand",
        "felt",
    ];
    for name in names {
        let needle = format!(r#"data-flight-anchor="{name}""#);
        assert!(html.contains(&needle), "missing anchor {name}");
    }
}

#[tokio::test]
async fn test_preview_shows_all_six_beat_hues() {
    let app = test_app().await;
    let html = body_string(get(&app, "/lastcall/preview").await).await;

    for hue in [
        "lc-beat-mint",
        "lc-beat-violet",
        "lc-beat-azure",
        "lc-beat-rose",
    ] {
        assert!(html.contains(hue), "missing {hue}");
    }
    assert!(
        html.matches("lc-beat-amber").count() >= 2,
        "expected lc-beat-amber at least twice (Draw and Deal)"
    );
}

#[tokio::test]
async fn test_preview_shows_every_plaque_state() {
    let app = test_app().await;
    let html = body_string(get(&app, "/lastcall/preview").await).await;

    for needle in [
        "is-locked",
        "is-drawing",
        "is-eliminated",
        "GHOST",
        "lc-lock-tick",
    ] {
        assert!(html.contains(needle), "missing {needle}");
    }

    // is-hit asserted on the plaque itself, not as a bare substring: the
    // literal text "is-hit" also appears on the page as the REPLAY button's
    // own `data-replay-state="is-hit"` attribute and in this group's note
    // prose, so `html.contains("is-hit")` would still pass even if the
    // `replacen` splice that actually adds the class to the plaque
    // (lc_preview.rs's plaque_with_is_hit) silently failed to match and
    // returned its input unchanged (plan-end review finding I1). This is
    // the spliced form's own prefix, so it can only be present if the
    // splice actually landed.
    assert!(
        html.contains(r#"class="lc-plaque is-hit "#),
        "is-hit did not land on a plaque's class attribute"
    );

    // The locked plaque's own markup — computed the same way the preview
    // does — must never carry a card identity. seats[2] (cara) is locked in
    // preview_state(); see the task report for the seat-index mapping.
    let view = drinkinggame::last_call::preview_state().public_view();
    let locked_html = drinkinggame::lc_render::player_plaque(&view.seats[2]);
    assert!(locked_html.contains("is-locked"));
    assert!(!locked_html.contains("data-card-id"));
    assert!(
        html.contains(&locked_html),
        "locked swatch not found on page"
    );

    // is-urgent (M3): beat_timer never emits it either — timer_with_is_urgent
    // splices it on the same way plaque_with_is_hit does for is-hit. Was
    // rendered but had no test at all before this fix.
    assert!(
        html.contains(r#"class="lc-timer is-urgent""#),
        "is-urgent did not land on the beat timer's class attribute"
    );
}

#[tokio::test]
async fn test_preview_shows_deck_stack_states() {
    let app = test_app().await;
    let html = body_string(get(&app, "/lastcall/preview").await).await;

    // The low/empty stacks — computed via deck_stack(...) the same way the
    // discard half below already does (M2), not bare substrings: `data-low`
    // and `data-empty` are also emitted, unrelated, on every deck stack's
    // sibling attributes, so a page-wide `html.contains("data-low")` would
    // still pass even if deck_stack stopped setting it for Wine specifically.
    // preview_state()'s fixture: Wine sits at 4 (< DECK_LOW_THRESHOLD, > 0),
    // Liquor at 0.
    let view = drinkinggame::last_call::preview_state().public_view();
    let low = drinkinggame::lc_render::deck_stack(drinkinggame::last_call::Deck::Wine, 4);
    assert!(low.contains("data-low"));
    assert!(html.contains(&low), "low deck stack not found on page");

    let empty = drinkinggame::lc_render::deck_stack(drinkinggame::last_call::Deck::Liquor, 0);
    assert!(empty.contains("data-empty"));
    assert!(empty.contains("RESHUFFLE"));
    assert!(html.contains(&empty), "empty deck stack not found on page");

    // A .lc-discard WITH data-count — not two independent substring checks,
    // which would pass even if discard_slot stopped emitting data-count
    // (every deck stack and #lc-hand also carry that attribute).
    let discard = drinkinggame::lc_render::discard_slot(view.discard_count);
    assert!(discard.contains("lc-discard"));
    assert!(discard.contains("data-count"));
    assert!(html.contains(&discard), "discard slot not found on page");
}

/// Plan-end review finding I3 (production side, human ruling): Groups 3 and
/// 7 must render the fourth alpha rung (`.lc-edge-strong`, `--lc-ink-99`)
/// side by side with the other three, for every deck — not the
/// `<span class="lc-preview-caption">no bound class in Plan A</span>`
/// placeholder Task 3 shipped instead when it correctly reported the gap
/// rather than inventing a class. `test_lastcall_css_ink_alpha_ladder_has_four_rungs`
/// pins the CSS binding; this pins that the preview page actually uses it.
#[tokio::test]
async fn test_preview_shows_all_four_ink_alpha_rungs() {
    let app = test_app().await;
    let html = body_string(get(&app, "/lastcall/preview").await).await;

    assert!(
        !html.contains("no bound class in Plan A"),
        "the fourth alpha rung's placeholder caption is still on the page"
    );

    for (class, min_count) in [
        ("lc-edge-subtle", 5),
        ("lc-edge-plaque", 5),
        ("lc-edge-back", 5),
        ("lc-edge-strong", 5),
    ] {
        let count = html.matches(class).count();
        assert!(
            count >= min_count,
            "expected {class} at least {min_count} times (once per deck), got {count}"
        );
    }
}

#[tokio::test]
async fn test_preview_shows_oversized_hand_split() {
    let app = test_app().await;
    let html = body_string(get(&app, "/lastcall/preview").await).await;

    // Bob's 12-card plaque hand (n - 7 = 5) and the isolated 30-card
    // hand_strip sample (n - 7 = 23).
    assert!(html.contains("+5"), "missing bob's +5 (12-card hand)");
    assert!(html.contains("+23"), "missing the 30-card sample's +23");

    // The isolated n = 8 sample sits exactly on the split boundary: 8
    // backs, no +n chip. Computed directly rather than scraped from the
    // whole page, since the page also renders plaques whose own hand
    // strips carry unrelated data-size="strip" backs.
    let n8 = drinkinggame::lc_render::hand_strip(&[drinkinggame::last_call::Deck::Beer], 8);
    assert_eq!(n8.matches(r#"data-size="strip""#).count(), 8);
    assert!(!n8.contains("lc-handstrip-more"));
    assert!(html.contains(&n8), "n=8 sample not found on page");
}

#[tokio::test]
async fn test_preview_tab_order_is_fixed() {
    let app = test_app().await;
    let html = body_string(get(&app, "/lastcall/preview").await).await;

    // At least the two dedicated side-by-side copies (HAND active, TABLE
    // active); the F.1 frame swatch embeds a third. Every occurrence must
    // keep the fixed order — a stronger check than "the two copies", not a
    // weaker one.
    let blocks: Vec<&str> = html.split(r#"class="lc-tabs""#).skip(1).collect();
    assert!(blocks.len() >= 2, "expected at least two tab-row copies");
    for block in blocks {
        let end = block.find("</div>").unwrap_or(block.len());
        let window = &block[..end];
        let hand = window.find("HAND").expect("HAND missing in tab row");
        let table = window.find("TABLE").expect("TABLE missing in tab row");
        let log = window.find("LOG").expect("LOG missing in tab row");
        assert!(hand < table, "HAND must come before TABLE");
        assert!(table < log, "TABLE must come before LOG");
    }
}

#[tokio::test]
async fn test_preview_has_the_felt_and_all_three_flight_directions() {
    let app = test_app().await;
    let html = body_string(get(&app, "/lastcall/preview").await).await;

    assert!(html.contains(r#"id="lc-felt""#));
    assert!(html.contains(r#"data-replay="draw""#));
    assert!(html.contains(r#"data-replay="play""#));
    assert!(html.contains(r#"data-replay="discard""#));
}

#[tokio::test]
async fn test_preview_script_delegates_to_the_motion_library() {
    let app = test_app().await;
    let html = body_string(get(&app, "/lastcall/preview").await).await;

    for needle in ["DOMContentLoaded", "htmx:afterSwap", "lcFlight", "lcAnchor"] {
        assert!(html.contains(needle), "missing {needle}");
    }
    assert!(!html.contains("@keyframes"));
    assert!(!html.contains("getBoundingClientRect"));
}

// -------------------------------------------------------------
// Last Call (Task 1): the cross-game arms, /lastcall/{start,vessel,handicap},
// and the entry redirect in room_page. Ring of Fire and 3 Man must come out
// of this untouched — test_room_page_unchanged_for_rof_and_three_man is the
// invariant this task exists to protect.
// -------------------------------------------------------------

use drinkinggame::last_call::LastCallState;

async fn lc_state_json(pool: &sqlx::SqlitePool, code: &str) -> String {
    let room = drinkinggame::db::get_open_room(pool, code).await.unwrap();
    drinkinggame::db::get_active_game(pool, room.id)
        .await
        .unwrap()
        .state_json
        .unwrap()
}

async fn lc_state(pool: &sqlx::SqlitePool, code: &str) -> LastCallState {
    LastCallState::from_json(&lc_state_json(pool, code).await)
}

/// Plan G, Task 3: strips the one `data-seq="N"` value a `/lastcall/hand`
/// fragment carries (on `#lc-hand` — see `lc_hand_pane`) so two fragments
/// that differ only in `st.seq` (bumped by pact formation, which has
/// nothing to do with what an uninvolved third party's own section shows)
/// compare equal. No regex dependency: `data-seq="` occurs exactly once in
/// the fragment, so a single find-and-splice is exact, not a heuristic.
fn without_seq(html: &str) -> String {
    let marker = r#"data-seq=""#;
    let Some(start) = html.find(marker) else {
        return html.to_string();
    };
    let value_start = start + marker.len();
    let Some(end_offset) = html[value_start..].find('"') else {
        return html.to_string();
    };
    let end = value_start + end_offset;
    format!("{}{}", &html[..value_start], &html[end..])
}

/// The Step 1 regression: without the `last_call` arms in `game.rs`'s three
/// panel builders, `current_panel`/`current_screen_panel`/
/// `current_room_panel` fall through to the Ring of Fire branch, and
/// `cards::parse_deck("")` (a Last Call game's `deck_order` is always "")
/// panics before a single SSE frame goes out.
#[tokio::test]
async fn test_lastcall_sse_snapshot_does_not_panic() {
    use futures::StreamExt;
    let app = test_app().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;
    let res = post_form(&app, &alice, &format!("/room/{code}/lastcall/start"), "").await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    let res = app
        .oneshot(
            Request::get(format!("/room/{code}/sse"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let mut body = res.into_body().into_data_stream();
    let frame = body.next().await.unwrap().unwrap();
    assert!(!frame.is_empty());
}

#[tokio::test]
async fn test_lastcall_start_requires_two_players() {
    let app = test_app().await;
    let alice = login(&app, "alice", "1234").await;
    let code = create_room(&app, &alice).await;

    let res = post_form(&app, &alice, &format!("/room/{code}/lastcall/start"), "").await;
    assert_eq!(res.status(), StatusCode::CONFLICT);
    assert!(body_string(res).await.contains("at least 2 players"));
}

#[tokio::test]
async fn test_lastcall_start_rejects_non_member() {
    let app = test_app().await;
    let alice = login(&app, "alice", "1234").await;
    let cara = login(&app, "cara", "1234").await;
    let code = create_room(&app, &alice).await;

    // cara never opened the room, so she isn't a member.
    let res = post_form(&app, &cara, &format!("/room/{code}/lastcall/start"), "").await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

/// Mirrors `test_tm_routes_reject_rof_games`: `/lastcall/vessel` posted
/// against an active Ring of Fire game must fail through `load_lc`'s
/// `WrongGameKind` path, not the form's own validation.
#[tokio::test]
async fn test_lastcall_routes_reject_rof_games() {
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

    let res = post_form(
        &app,
        &alice,
        &format!("/room/{code}/lastcall/vessel"),
        "deck=liquor&container=pint%20glass",
    )
    .await;
    assert_eq!(res.status(), StatusCode::CONFLICT);
    assert!(body_string(res).await.contains("belongs to the other game"));
}

#[tokio::test]
async fn test_lastcall_vessel_sets_deck_constant_pulls() {
    let (app, pool) = test_app_with_pool().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;
    post_form(&app, &alice, &format!("/room/{code}/lastcall/start"), "").await;

    let res = post_form(
        &app,
        &alice,
        &format!("/room/{code}/lastcall/vessel"),
        "deck=liquor&container=pint%20glass",
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    let alice_player = drinkinggame::db::get_player_by_name(&pool, "alice")
        .await
        .unwrap();
    let st = lc_state(&pool, &code).await;
    let seat = st.seat_of(alice_player.id).unwrap();
    // pulls_max is the deck constant (liquor = 4), never anything derived
    // from the deliberately contradictory "pint glass" container label.
    assert_eq!(st.players[seat].vessels[0].pulls_max, 4);
    assert_eq!(st.players[seat].hand.len(), 5); // F6 opener, not the old 4-card deal
}

#[tokio::test]
async fn test_lastcall_vessel_rejects_unknown_deck() {
    let (app, pool) = test_app_with_pool().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;
    post_form(&app, &alice, &format!("/room/{code}/lastcall/start"), "").await;
    let before = lc_state_json(&pool, &code).await;

    let res = post_form(
        &app,
        &alice,
        &format!("/room/{code}/lastcall/vessel"),
        "deck=absinthe&container=glass",
    )
    .await;
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(lc_state_json(&pool, &code).await, before);
}

/// Fix round 1 (Plan E Task 1 review): `lc_vessel_handler` used to blanket-422
/// every `set_vessel` error, so D15's `WrongBeat`/`NotAlive` came back 422
/// while the same variants from `lc_handicap_handler` mapped through `map_lc`
/// to 409/403. Both handlers must now agree — pins the statuses directly
/// against what `map_lc` gives those variants everywhere else.
#[tokio::test]
async fn test_lastcall_vessel_maps_wrong_beat_and_not_alive_like_map_lc() {
    let (app, pool) = test_app_with_pool().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;
    post_form(&app, &alice, &format!("/room/{code}/lastcall/start"), "").await;
    let game_id = lc_game_id(&pool, &code).await;

    // WrongBeat: D15 gates set_vessel to Beat::Draw.
    let mut st = lc_state(&pool, &code).await;
    st.beat = Beat::Lock;
    drinkinggame::db::set_game_state(&pool, game_id, &st.to_json()).await;

    let res = post_form(
        &app,
        &alice,
        &format!("/room/{code}/lastcall/vessel"),
        "deck=liquor&container=glass",
    )
    .await;
    assert_eq!(res.status(), StatusCode::CONFLICT); // map_lc's WrongBeat -> 409

    // NotAlive: eliminate alice, beat back to Draw so only NotAlive fires.
    let mut st = lc_state(&pool, &code).await;
    st.beat = Beat::Draw;
    st.players[0].status = drinkinggame::last_call::Status::Eliminated;
    drinkinggame::db::set_game_state(&pool, game_id, &st.to_json()).await;

    let res = post_form(
        &app,
        &alice,
        &format!("/room/{code}/lastcall/vessel"),
        "deck=liquor&container=glass",
    )
    .await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN); // map_lc's NotAlive -> 403
}

/// Spec §2, item 2: handicaps are set by the table, not the player they
/// belong to. bob setting alice's handicap (not his own) must succeed — a
/// future "only you may set yours" regression would fail this test.
#[tokio::test]
async fn test_lastcall_handicap_is_not_owner_scoped() {
    let (app, pool) = test_app_with_pool().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;
    post_form(&app, &alice, &format!("/room/{code}/lastcall/start"), "").await;

    let alice_player = drinkinggame::db::get_player_by_name(&pool, "alice")
        .await
        .unwrap();
    let res = post_form(
        &app,
        &bob,
        &format!("/room/{code}/lastcall/handicap"),
        &format!("target={}&handicap_pct=150", alice_player.id),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    let st = lc_state(&pool, &code).await;
    let seat = st.seat_of(alice_player.id).unwrap();
    assert_eq!(st.players[seat].handicap_pct, 150);
}

/// `handicap_pct=301`/`=24` extract fine into `u16` and fail `set_handicap`'s
/// own range check (422 via `LcError::BadHandicap`); `=-5`/`=abc` never reach
/// that check at all — axum's `Form<HandicapForm>` extractor rejects them
/// while parsing `u16` and returns 422 on its own (verified against the real
/// extractor, not assumed from the brief's "422 at extraction" wording).
/// Either way, no case ever calls `set_handicap`, so state is unchanged
/// throughout.
#[tokio::test]
async fn test_lastcall_handicap_rejects_out_of_range() {
    let (app, pool) = test_app_with_pool().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;
    post_form(&app, &alice, &format!("/room/{code}/lastcall/start"), "").await;
    let alice_player = drinkinggame::db::get_player_by_name(&pool, "alice")
        .await
        .unwrap();
    let before = lc_state_json(&pool, &code).await;

    for pct in ["301", "24", "-5", "abc"] {
        let res = post_form(
            &app,
            &alice,
            &format!("/room/{code}/lastcall/handicap"),
            &format!("target={}&handicap_pct={pct}", alice_player.id),
        )
        .await;
        assert_eq!(
            res.status(),
            StatusCode::UNPROCESSABLE_ENTITY,
            "handicap_pct={pct}"
        );
    }
    assert_eq!(lc_state_json(&pool, &code).await, before);
}

/// D19 gap-close (Plan E Task 1): `set_handicap` is Draw-beat-gated, so a
/// handicap POST after Lock must map to `LcError::WrongBeat`'s "not now"
/// (409), not fold into the same 422 bucket as an actual out-of-range
/// percentage — see `lc_handicap_handler`'s doc comment.
#[tokio::test]
async fn test_lastcall_handicap_after_lock_is_conflict() {
    let (app, pool) = test_app_with_pool().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;
    post_form(&app, &alice, &format!("/room/{code}/lastcall/start"), "").await;
    let alice_player = drinkinggame::db::get_player_by_name(&pool, "alice")
        .await
        .unwrap();

    let mut st = lc_state(&pool, &code).await;
    st.beat = Beat::Lock;
    let game_id = lc_game_id(&pool, &code).await;
    drinkinggame::db::set_game_state(&pool, game_id, &st.to_json()).await;

    let res = post_form(
        &app,
        &alice,
        &format!("/room/{code}/lastcall/handicap"),
        &format!("target={}&handicap_pct=150", alice_player.id),
    )
    .await;
    assert_eq!(res.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn test_room_page_redirects_to_lastcall_shell() {
    let app = test_app().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;
    post_form(&app, &alice, &format!("/room/{code}/lastcall/start"), "").await;

    let res = app
        .oneshot(
            Request::get(format!("/room/{code}"))
                .header(header::COOKIE, &alice)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        res.headers()[header::LOCATION],
        format!("/room/{code}/lastcall")
    );

    // Same redirect, mounted at a non-empty base path — the generated
    // Location must carry the base_path prefix.
    let app = test_app_with_base("/drinks").await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;
    post_form(&app, &alice, &format!("/room/{code}/lastcall/start"), "").await;

    let res = app
        .oneshot(
            Request::get(format!("/room/{code}"))
                .header(header::COOKIE, &alice)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        res.headers()[header::LOCATION],
        format!("/drinks/room/{code}/lastcall")
    );
}

/// The invariant this whole task exists to protect: a room with Ring of Fire
/// or 3 Man active — or nothing active at all — must render the room page
/// exactly as before, with no redirect and the game panel in place.
#[tokio::test]
async fn test_room_page_unchanged_for_rof_and_three_man() {
    let app = test_app().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;

    // No active game: 200, all three start cards, no redirect.
    let res = app
        .clone()
        .oneshot(
            Request::get(format!("/room/{code}"))
                .header(header::COOKIE, &alice)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let html = body_string(res).await;
    assert!(html.contains(r#"data-pane="game""#));
    assert!(html.contains("Ring of Fire"));
    assert!(html.contains("3 Man"));
    assert!(html.contains("Last Call"));

    // Ring of Fire active: 200, game panel present, no redirect.
    post_form(
        &app,
        &alice,
        &format!("/room/{code}/game/start"),
        "preset_id=1",
    )
    .await;
    let res = app
        .clone()
        .oneshot(
            Request::get(format!("/room/{code}"))
                .header(header::COOKIE, &alice)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert!(body_string(res).await.contains(r#"data-pane="game""#));
    post_form(&app, &alice, &format!("/room/{code}/game/end"), "").await;

    // 3 Man active: 200, game panel present, no redirect.
    post_form(&app, &alice, &format!("/room/{code}/tm/start"), "").await;
    let res = app
        .oneshot(
            Request::get(format!("/room/{code}"))
                .header(header::COOKIE, &alice)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert!(body_string(res).await.contains(r#"data-pane="game""#));
}

#[tokio::test]
async fn test_room_page_seats_late_joiner_in_lastcall() {
    let (app, pool) = test_app_with_pool().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let cara = login(&app, "cara", "1234").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;
    post_form(&app, &alice, &format!("/room/{code}/lastcall/start"), "").await;

    // cara never visited this room before: opening it seats her in
    // LastCallState too, not just room_players, and redirects her straight
    // to the shell.
    let res = app
        .oneshot(
            Request::get(format!("/room/{code}"))
                .header(header::COOKIE, &cara)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        res.headers()[header::LOCATION],
        format!("/room/{code}/lastcall")
    );

    let cara_player = drinkinggame::db::get_player_by_name(&pool, "cara")
        .await
        .unwrap();
    let st = lc_state(&pool, &code).await;
    assert_eq!(st.players.len(), 3);
    let seat = st.seat_of(cara_player.id).unwrap();
    assert_eq!(seat, 2);
    assert_eq!(st.players[seat].hp, 15);
    assert_eq!(st.players[seat].handicap_pct, 100);
}

#[tokio::test]
async fn test_lastcall_start_rejects_second_game() {
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

    let res = post_form(&app, &alice, &format!("/room/{code}/lastcall/start"), "").await;
    assert_eq!(res.status(), StatusCode::CONFLICT);
    assert!(body_string(res).await.contains("already running"));
}

// -------------------------------------------------------------
// Last Call (Task 2): the F.1 phone shell (`GET /room/{code}/lastcall`) and
// the private hand fragment (`GET /room/{code}/lastcall/hand`). spec §6.1's
// constraint — the hand route takes no player identifier of any kind, so
// identity comes from the session cookie alone — is the property this task
// exists to establish; test_lastcall_hand_is_private and
// test_lastcall_hand_route_takes_no_player_input assert it behaviourally.
// -------------------------------------------------------------

async fn get_hand(app: &Router, cookie: &str, code: &str) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::get(format!("/room/{code}/lastcall/hand"))
                .header(header::COOKIE, cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn get_shell(app: &Router, cookie: &str, code: &str) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::get(format!("/room/{code}/lastcall"))
                .header(header::COOKIE, cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
}

#[tokio::test]
async fn test_lastcall_shell_renders_fixed_tab_order() {
    let app = test_app().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;
    post_form(&app, &alice, &format!("/room/{code}/lastcall/start"), "").await;

    let res = get_shell(&app, &alice, &code).await;
    assert_eq!(res.status(), StatusCode::OK);
    let html = body_string(res).await;

    let hand_i = html.find(r#"data-lc-tab="hand""#).unwrap();
    let table_i = html.find(r#"data-lc-tab="table""#).unwrap();
    let log_i = html.find(r#"data-lc-tab="log""#).unwrap();
    assert!(hand_i < table_i, "hand must precede table");
    assert!(table_i < log_i, "table must precede log");

    assert!(html.contains(r#"href="/assets/lastcall.css""#));
    assert!(!html.contains("game.css"));
}

#[tokio::test]
async fn test_lastcall_shell_requires_membership() {
    let app = test_app().await;
    let alice = login(&app, "alice", "1234").await;
    let cara = login(&app, "cara", "1234").await; // never opens the room: not a member
    let code = create_room(&app, &alice).await;

    let res = get_shell(&app, &cara, &code).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // No cookie at all: the same PlayerSession redirect/rejection the crate
    // already produces for a bare /room/{code} GET.
    let res = app
        .oneshot(
            Request::get(format!("/room/{code}/lastcall"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
}

/// The one that matters (spec §8). A live two-session test, alongside the
/// structural guarantee (the handler signature itself) that there is no
/// input naming a player.
#[tokio::test]
async fn test_lastcall_hand_is_private() {
    let app = test_app().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;
    post_form(&app, &alice, &format!("/room/{code}/lastcall/start"), "").await;
    post_form(
        &app,
        &alice,
        &format!("/room/{code}/lastcall/vessel"),
        "deck=beer&container=50cl%20can",
    )
    .await;
    post_form(
        &app,
        &bob,
        &format!("/room/{code}/lastcall/vessel"),
        "deck=wine&container=15cl%20glass",
    )
    .await;

    let alice_hand = body_string(get_hand(&app, &alice, &code).await).await;
    assert!(alice_hand.contains("beer-01"));
    assert!(!alice_hand.contains("wine-01"));

    let bob_hand = body_string(get_hand(&app, &bob, &code).await).await;
    assert!(bob_hand.contains("wine-01"));
    assert!(!bob_hand.contains("beer-01"));

    let res = app
        .oneshot(
            Request::get(format!("/room/{code}/lastcall/hand"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_ne!(res.status(), StatusCode::OK);
}

/// Asserts the §6.1 constraint behaviourally, not just by signature:
/// appending a caller-supplied player identifier to the query string must
/// change nothing about the response.
#[tokio::test]
async fn test_lastcall_hand_route_takes_no_player_input() {
    let (app, pool) = test_app_with_pool().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;
    post_form(&app, &alice, &format!("/room/{code}/lastcall/start"), "").await;
    post_form(
        &app,
        &alice,
        &format!("/room/{code}/lastcall/vessel"),
        "deck=beer&container=50cl%20can",
    )
    .await;
    let bob_player = drinkinggame::db::get_player_by_name(&pool, "bob")
        .await
        .unwrap();

    let baseline = body_string(get_hand(&app, &alice, &code).await).await;

    let with_player_id = body_string(
        app.clone()
            .oneshot(
                Request::get(format!(
                    "/room/{code}/lastcall/hand?player_id={}",
                    bob_player.id
                ))
                .header(header::COOKIE, &alice)
                .body(Body::empty())
                .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(baseline, with_player_id);

    let with_target = body_string(
        app.oneshot(
            Request::get(format!(
                "/room/{code}/lastcall/hand?target={}",
                bob_player.id
            ))
            .header(header::COOKIE, &alice)
            .body(Body::empty())
            .unwrap(),
        )
        .await
        .unwrap(),
    )
    .await;
    assert_eq!(baseline, with_target);
}

/// Plan G, Task 3: the fragment half of the mandatory privacy property for
/// pacts — `pacts_section_html` reads `pacts`/`pact_offers`/`pact_barred`,
/// none of which `PublicView` ever projects (G13), so the ONLY place any of
/// it may render is the viewer's own `/lastcall/hand` fragment. Mirrors
/// `test_hand_fragment_carries_only_the_viewers_armed_cards`'s shape (Task
/// 4's fixture rig), but for pacts: state is hand-rolled with real player
/// ids so `offer_pact`/`accept_pact` — which validate against `seat_of` —
/// can be called directly on it before persisting.
#[tokio::test]
async fn test_the_hand_fragment_shows_only_the_viewers_own_pact() {
    let (app, pool) = test_app_with_pool().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let cara = login(&app, "cara", "1234").await;
    let dave = login(&app, "dave", "5678").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;
    room_page_html(&app, &cara, &code).await;
    room_page_html(&app, &dave, &code).await;
    post_form(&app, &alice, &format!("/room/{code}/lastcall/start"), "").await;

    let alice_id = drinkinggame::db::get_player_by_name(&pool, "alice")
        .await
        .unwrap()
        .id;
    let bob_id = drinkinggame::db::get_player_by_name(&pool, "bob")
        .await
        .unwrap()
        .id;
    let cara_id = drinkinggame::db::get_player_by_name(&pool, "cara")
        .await
        .unwrap()
        .id;
    let dave_id = drinkinggame::db::get_player_by_name(&pool, "dave")
        .await
        .unwrap()
        .id;
    let room = drinkinggame::db::get_open_room(&pool, &code).await.unwrap();
    let game_id = drinkinggame::db::get_active_game(&pool, room.id)
        .await
        .unwrap()
        .id;

    // alice(0)/bob(1)/cara(2)/dave(3), vessels registered, at Diplomacy —
    // the same shape `LastCallState::new` + `set_vessel` builds elsewhere,
    // with real ids so `offer_pact`/`accept_pact` (which resolve seats via
    // `seat_of`) work against it.
    let fresh_rig = || {
        let mut st = LastCallState::new(
            vec![
                (alice_id, "alice".into()),
                (bob_id, "bob".into()),
                (cara_id, "cara".into()),
                (dave_id, "dave".into()),
            ],
            1,
        );
        st.set_vessel(alice_id, Deck::Beer, "can").unwrap();
        st.set_vessel(bob_id, Deck::Cider, "bottle").unwrap();
        st.set_vessel(cara_id, Deck::Soft, "glass").unwrap();
        st.set_vessel(dave_id, Deck::Liquor, "shot").unwrap();
        st.beat = Beat::Diplomacy;
        st
    };

    // The comparison rig (Task 4's `without_seq` property): same seats and
    // vessels, but no pact ever offered or formed.
    let baseline = fresh_rig();
    drinkinggame::db::set_game_state(&pool, game_id, &baseline.to_json()).await;
    let cara_baseline = body_string(get_hand(&app, &cara, &code).await).await;

    let mut st = fresh_rig();
    st.offer_pact(alice_id, 1).unwrap(); // alice (seat 0) -> bob (seat 1)
    st.accept_pact(bob_id, 0).unwrap(); // bob accepts alice's offer
    drinkinggame::db::set_game_state(&pool, game_id, &st.to_json()).await;

    let alice_hand = body_string(get_hand(&app, &alice, &code).await).await;
    assert!(alice_hand.contains("PACT WITH BOB"));

    let bob_hand = body_string(get_hand(&app, &bob, &code).await).await;
    assert!(bob_hand.contains("PACT WITH ALICE"));

    let cara_hand = body_string(get_hand(&app, &cara, &code).await).await;
    assert!(!cara_hand.contains("PACT WITH"));
    assert!(!cara_hand.contains("lc-pact-standing"));
    assert_eq!(
        without_seq(&cara_hand),
        without_seq(&cara_baseline),
        "cara's fragment must be identical whether or not alice and bob pacted"
    );
}

#[tokio::test]
async fn test_lastcall_hand_rejects_wrong_game_kind() {
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

    let res = get_hand(&app, &alice, &code).await;
    assert_eq!(res.status(), StatusCode::CONFLICT);
    assert!(body_string(res).await.contains("belongs to the other game"));
}

/// Spec §2, item 2, at the page level: alice's shell shows a settable
/// handicap row for bob as well as for herself, not just her own.
#[tokio::test]
async fn test_lastcall_shell_shows_all_handicap_rows() {
    let (app, pool) = test_app_with_pool().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;
    post_form(&app, &alice, &format!("/room/{code}/lastcall/start"), "").await;

    let alice_player = drinkinggame::db::get_player_by_name(&pool, "alice")
        .await
        .unwrap();
    let bob_player = drinkinggame::db::get_player_by_name(&pool, "bob")
        .await
        .unwrap();

    let html = body_string(get_shell(&app, &alice, &code).await).await;
    assert_eq!(html.matches("lc-setup-row").count(), 2);
    assert!(html.contains(&format!(r#"value="{}""#, alice_player.id)));
    assert!(html.contains(&format!(r#"value="{}""#, bob_player.id)));
}

// -------------------------------------------------------------
// Last Call (Plan C Task 2): the hand group — HandWheel, armed column, cost
// rail — replaces the throwaway plain card list inside `#lc-hand`. Seeded by
// hand-rolling a `LastCallState` and persisting it via `set_game_state` (the
// `tm_handoff_gating` pattern), because `arm`/`lock_in` were not implemented
// yet at the time this section was written — this was the only way to get
// an armed/locked hand onto the wire at that point in the plan. Both routes
// exist now (Plan E Task 1); the tests below still hand-roll state directly
// rather than switching to the real routes, but any fixture claiming
// `locked = true` must now respect the invariant `lock_in` actually
// establishes — a staged card lives in `locked_plays`, not `armed`, once its
// seat is locked (Plan E Task 4).
// -------------------------------------------------------------

/// The privacy property the armed column exists to uphold, restated at the
/// transport layer (mirrors `test_lastcall_hand_is_private` for the hand
/// itself): A's fragment carries A's armed card inside its own `.lc-armed`
/// block and never leaks B's, and vice versa.
#[tokio::test]
async fn test_hand_fragment_carries_only_the_viewers_armed_cards() {
    let (app, pool) = test_app_with_pool().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;
    post_form(&app, &alice, &format!("/room/{code}/lastcall/start"), "").await;

    let alice_id = drinkinggame::db::get_player_by_name(&pool, "alice")
        .await
        .unwrap()
        .id;
    let bob_id = drinkinggame::db::get_player_by_name(&pool, "bob")
        .await
        .unwrap()
        .id;
    let room = drinkinggame::db::get_open_room(&pool, &code).await.unwrap();
    let game_id = drinkinggame::db::get_active_game(&pool, room.id)
        .await
        .unwrap()
        .id;

    let mut st = LastCallState::new(vec![(alice_id, "alice".into()), (bob_id, "bob".into())], 1);
    st.players[0].armed = vec![drinkinggame::last_call::ArmedCard {
        card: drinkinggame::lc_cards::card_by_id("beer-01").unwrap(),
        target: None,
    }];
    // bob is LOCKED — the real invariant `lock_in` establishes moves a
    // staged card out of `armed` and into `locked_plays` (Plan E Task 4:
    // `hand_pane_html` now reads `staged_for(seat)` for a locked viewer, not
    // `armed`, closing the "LOCKED 0" seam the Plan D review flagged), so
    // this fixture models that directly rather than seeding `armed` +
    // `locked = true` together, a combination `lock_in` never actually
    // produces.
    st.locked_plays = vec![drinkinggame::last_call::Play {
        card: drinkinggame::lc_cards::card_by_id("cider-01").unwrap(),
        source_seat: 1,
        target: None,
        paid_from: drinkinggame::last_call::Deck::Cider,
        order_key: 0,
    }];
    st.players[1].locked = true;
    drinkinggame::db::set_game_state(&pool, game_id, &st.to_json()).await;

    // Both hands are empty (only `armed` was seeded), so the pane's HandWheel
    // never renders and the cost rail is the next block after the armed
    // column — bounding the "inside an lc-armed block" check between them.
    let alice_hand = body_string(get_hand(&app, &alice, &code).await).await;
    let armed_start = alice_hand.find("lc-armed").expect("lc-armed missing");
    let rail_start = alice_hand.find("lc-costrail").expect("lc-costrail missing");
    assert!(rail_start > armed_start);
    assert!(alice_hand[armed_start..rail_start].contains("beer-01"));
    assert!(alice_hand.contains("ARMED 1"));
    assert!(!alice_hand.contains("cider-01"));

    let bob_hand = body_string(get_hand(&app, &bob, &code).await).await;
    let armed_start = bob_hand.find("lc-armed").expect("lc-armed missing");
    let rail_start = bob_hand.find("lc-costrail").expect("lc-costrail missing");
    assert!(rail_start > armed_start);
    assert!(bob_hand[armed_start..rail_start].contains("cider-01"));
    assert!(bob_hand.contains("LOCKED 1"));
    assert!(bob_hand.contains(" data-locked"));
    assert!(!bob_hand.contains("beer-01"));
}

/// The rail's `pull_cost` math is not cosmetic: the same card in two
/// viewers' hands, priced through their own `handicap_pct`, must produce two
/// different `data-pull-cost` values on their two private fragments.
#[tokio::test]
async fn test_hand_fragment_prices_the_rail_by_the_viewers_own_handicap() {
    let (app, pool) = test_app_with_pool().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;
    post_form(&app, &alice, &format!("/room/{code}/lastcall/start"), "").await;

    let alice_id = drinkinggame::db::get_player_by_name(&pool, "alice")
        .await
        .unwrap()
        .id;
    let bob_id = drinkinggame::db::get_player_by_name(&pool, "bob")
        .await
        .unwrap()
        .id;
    let room = drinkinggame::db::get_open_room(&pool, &code).await.unwrap();
    let game_id = drinkinggame::db::get_active_game(&pool, room.id)
        .await
        .unwrap()
        .id;

    let card = drinkinggame::lc_cards::card_by_id("wine-01").unwrap();
    assert_eq!(card.cost, 2, "fixture assumes wine-01 costs 2");

    let mut st = LastCallState::new(vec![(alice_id, "alice".into()), (bob_id, "bob".into())], 1);
    st.players[0].hand = vec![card.clone()];
    st.players[0].handicap_pct = 300;
    st.players[1].hand = vec![card];
    st.players[1].handicap_pct = 100;
    drinkinggame::db::set_game_state(&pool, game_id, &st.to_json()).await;

    let alice_hand = body_string(get_hand(&app, &alice, &code).await).await;
    assert!(alice_hand.contains(r#"data-pull-cost="6""#));

    let bob_hand = body_string(get_hand(&app, &bob, &code).await).await;
    assert!(bob_hand.contains(r#"data-pull-cost="2""#));
}

/// I1 (Plan H review): the rail must price through the SAME cost-halving
/// seam as the engine's charge during `happy-hour`, over the real
/// `hand_pane_html` route — not just the `cost_rail`/`HandGroupView` unit
/// tests, which call the builder directly and would stay green even if
/// `hand_pane_html`'s wiring (`halved: st.cost_halved()`) were dropped or
/// hardcoded to `false` again. This is the regression the pre-fix rail
/// actually had: unhalved bars while `arm`/`lock_in`/the reveal charge/the
/// DRINK chip all agreed at the halved number.
#[tokio::test]
async fn test_hand_fragment_rail_halves_under_happy_hour_like_the_engine_charge() {
    let (app, pool) = test_app_with_pool().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;
    post_form(&app, &alice, &format!("/room/{code}/lastcall/start"), "").await;

    let alice_id = drinkinggame::db::get_player_by_name(&pool, "alice")
        .await
        .unwrap()
        .id;
    let bob_id = drinkinggame::db::get_player_by_name(&pool, "bob")
        .await
        .unwrap()
        .id;
    let room = drinkinggame::db::get_open_room(&pool, &code).await.unwrap();
    let game_id = drinkinggame::db::get_active_game(&pool, room.id)
        .await
        .unwrap()
        .id;

    let card = drinkinggame::lc_cards::card_by_id("beer-02").unwrap();
    assert_eq!(card.cost, 2, "fixture assumes beer-02 costs 2");

    let mut st = LastCallState::new(vec![(alice_id, "alice".into()), (bob_id, "bob".into())], 1);
    st.players[0].hand = vec![card];
    st.event = Some("happy-hour".into());
    drinkinggame::db::set_game_state(&pool, game_id, &st.to_json()).await;

    let alice_hand = body_string(get_hand(&app, &alice, &code).await).await;
    // Unhalved would be data-pull-cost="2" (cost 2, handicap 100) — the
    // pre-fix rail's number. Halved is "1", matching effective_pull_cost.
    assert!(alice_hand.contains(r#"data-card-id="beer-02" data-cost="2" data-pull-cost="1""#));
    assert!(!alice_hand.contains(r#"data-pull-cost="2""#));
}

// -------------------------------------------------------------
// Last Call (Task 3): the SSE contract (`lcpublic`/`lctick`) and the
// client-side stale-drop rule. The privacy invariant this task exists to
// protect — a broadcast fragment can never carry unrevealed card identity,
// because it's rendered from `PublicView` — is asserted at the transport
// layer by test_lcpublic_never_carries_hand_cards.
// -------------------------------------------------------------

/// With a Last Call game active, the connect-time snapshot carries a fifth
/// `lcpublic` frame (after the usual leaderboard/game/screen/room four),
/// rendered from `PublicView` and carrying the §7.8 hand-region seq floor.
/// Reads by content (`read_sse_until`), not by counting `next()` calls — its
/// own doc comment warns that a body chunk is not guaranteed to be one SSE
/// event, only that the last_call `game`/`screen` panels are still Task 1's
/// tiny placeholders today, which happens to make a fixed-count drain safe.
#[tokio::test]
async fn test_lastcall_sse_snapshot_includes_lcpublic() {
    let app = test_app().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;
    post_form(&app, &alice, &format!("/room/{code}/lastcall/start"), "").await;

    let res = get(&app, &format!("/room/{code}/sse")).await;
    let mut body = res.into_body().into_data_stream();
    let seen = read_sse_until(&mut body, "event: lcpublic").await;
    let lcpublic_at = seen.find("event: lcpublic").unwrap();
    let frame = &seen[lcpublic_at..];
    assert!(frame.contains("data-lc-public"), "{frame}");
    assert!(frame.contains("data-seq="), "{frame}");
}

/// The regression a misplaced `.chain` would cause: landing the `lcpublic`
/// frame inside the existing four rather than after them. A Ring of Fire
/// room must still emit exactly the same four frames it always has, in the
/// same order, with none of them carrying `lcpublic`.
///
/// Filtered entirely by event NAME via `read_sse_until` (content search),
/// never by counting `next()` chunks or indexing into them — a body chunk is
/// not guaranteed to be one SSE event, and the brief's own constraint is
/// "filter by event name, never index positionally" (fix round 1, Important
/// 3: this test used to read exactly four `next()` chunks and assert
/// `frames[3].contains("event: room")`).
///
/// Plan-end review finding M1: content-matching for `room` alone only
/// catches a *misplaced* chain — it does not assert-catch a fifth frame
/// emitted *unconditionally* (regardless of game kind). Bound the wait
/// instead: give the stream 200ms after the four known frames to produce
/// anything else, and require that if it does, it still isn't `lcpublic`. A
/// timeout (no frame at all) is the expected, passing outcome for this room.
#[tokio::test]
async fn test_rof_sse_snapshot_has_no_lcpublic() {
    use futures::StreamExt;
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

    let res = get(&app, &format!("/room/{code}/sse")).await;
    let mut body = res.into_body().into_data_stream();
    let seen = read_sse_until(&mut body, "event: room").await;
    assert!(!seen.contains("lcpublic"), "{seen}");
    for name in ["leaderboard", "game", "screen", "room"] {
        let marker = format!("event: {name}");
        assert_eq!(
            seen.matches(&marker).count(),
            1,
            "expected exactly one `{marker}` frame in the snapshot: {seen}"
        );
    }
    assert_eq!(
        seen.matches("event: ").count(),
        4,
        "Ring of Fire's connect-time snapshot must stay four frames total: {seen}"
    );

    // No fifth frame is ever sent for a Ring of Fire room, so a bounded
    // timeout — not an unbounded `next()` — is what makes this assertion
    // safe to run at all.
    match tokio::time::timeout(std::time::Duration::from_millis(200), body.next()).await {
        Err(_) => {}   // timed out waiting: correct, nothing more was sent.
        Ok(None) => {} // stream ended: also fine.
        Ok(Some(chunk)) => {
            let frame = String::from_utf8(chunk.unwrap().to_vec()).unwrap();
            assert!(
                !frame.contains("lcpublic"),
                "an unconditionally emitted fifth frame must still not be \
                 lcpublic on a Ring of Fire room: {frame}"
            );
        }
    }
}

/// `persist_and_broadcast_lc` fires `broadcast_room` before `broadcast_lc`,
/// so the very next frame after a vessel POST is `room`, not `lcpublic` —
/// reading by content up to the `lctick` marker (rather than asserting
/// positionally, or counting `next()` calls) is what makes this test robust
/// to that ordering and to however the chunks happen to split.
#[tokio::test]
async fn test_lastcall_vessel_broadcasts_public_and_tick() {
    let app = test_app().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;
    post_form(&app, &alice, &format!("/room/{code}/lastcall/start"), "").await;

    let res = get(&app, &format!("/room/{code}/sse")).await;
    let mut body = res.into_body().into_data_stream();
    let snapshot = read_sse_until(&mut body, "event: lcpublic").await;
    let snapshot_frame = &snapshot[snapshot.find("event: lcpublic").unwrap()..];
    let after = snapshot_frame.split("data-seq=\"").nth(1).unwrap();
    let snapshot_seq: u64 = after
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect::<String>()
        .parse()
        .unwrap();

    post_form(
        &app,
        &bob,
        &format!("/room/{code}/lastcall/vessel"),
        "deck=liquor&container=pint%20glass",
    )
    .await;

    let seen = read_sse_until(&mut body, "event: lctick").await;
    let lcpublic_at = seen.find("event: lcpublic").expect("lcpublic must arrive");
    let lctick_at = seen.find("event: lctick").expect("lctick must arrive");
    assert!(
        lcpublic_at < lctick_at,
        "lcpublic must arrive before lctick: {seen}"
    );

    let tick_frame = &seen[lctick_at..];
    let seq: u64 = tick_frame
        .split("data: ")
        .nth(1)
        .unwrap()
        .lines()
        .next()
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert!(
        seq > snapshot_seq,
        "tick seq {seq} must exceed snapshot seq {snapshot_seq}"
    );
}

/// The privacy assertion at the transport layer: `LcPublic` is built from
/// `PublicView`, which by construction cannot carry unrevealed card
/// identity (spec §3.4) — this checks the wire, not just the type, so a
/// future change that widens the broadcast to raw `LastCallState` would
/// fail here even if it still compiled. Checks every frame on the stream
/// (not just `lcpublic` ones) — the spectator screen subscribes to the
/// whole thing, so a leak via `room`/`game`/`screen` would be just as real.
///
/// `set_vessel` (`last_call.rs`) pushes the chosen deck's curated opening
/// hand into the player's hand — `crate::lc_cards::opening_hand(deck)` —
/// so registering `beer` deterministically puts `beer-01` (among the
/// opener's other four cards) in alice's hand with no dependence on the rng
/// seed. That's what makes `beer-01` etc. a real needle rather than an
/// assumed one: the positive control below fetches alice's own hand
/// fragment and confirms `beer-01` is actually there before trusting its
/// absence from the broadcast frames as meaningful.
#[tokio::test]
async fn test_lcpublic_never_carries_hand_cards() {
    let app = test_app().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;
    post_form(&app, &alice, &format!("/room/{code}/lastcall/start"), "").await;

    let res = get(&app, &format!("/room/{code}/sse")).await;
    let mut body = res.into_body().into_data_stream();
    let mut seen = read_sse_until(&mut body, "event: lcpublic").await;

    post_form(
        &app,
        &alice,
        &format!("/room/{code}/lastcall/vessel"),
        "deck=beer&container=pint",
    )
    .await;
    seen.push_str(&read_sse_until(&mut body, "event: lctick").await);

    post_form(
        &app,
        &bob,
        &format!("/room/{code}/lastcall/vessel"),
        "deck=wine&container=glass",
    )
    .await;
    seen.push_str(&read_sse_until(&mut body, "event: lctick").await);

    // Positive control: prove the needle is real before trusting its
    // absence. alice registered "beer", so her own hand fragment must
    // contain beer-01 — if it didn't, the absence check below would be
    // vacuous.
    let hand_html = body_string(get_hand(&app, &alice, &code).await).await;
    assert!(
        hand_html.contains("beer-01"),
        "sanity check failed: alice's own hand should contain beer-01 — {hand_html}"
    );

    for id in ["beer-01", "cider-01", "wine-01", "liquor-01", "soft-01"] {
        assert!(!seen.contains(id), "broadcast stream leaked {id}: {seen}");
    }
}

/// The EventSource empty-data-buffer pitfall (WHATWG SSE spec): a named
/// event with an empty `data:` field is silently dropped by the browser
/// parser. `seq` is a `u64`, so `seq.to_string()` can never be empty — this
/// asserts that holds on the actual wire frame, not just by inspection.
#[tokio::test]
async fn test_lctick_payload_is_never_empty() {
    let app = test_app().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;
    post_form(&app, &alice, &format!("/room/{code}/lastcall/start"), "").await;

    let res = get(&app, &format!("/room/{code}/sse")).await;
    let mut body = res.into_body().into_data_stream();
    read_sse_until(&mut body, "event: lcpublic").await; // drain the snapshot

    post_form(
        &app,
        &alice,
        &format!("/room/{code}/lastcall/vessel"),
        "deck=liquor&container=pint%20glass",
    )
    .await;

    let seen = read_sse_until(&mut body, "event: lctick").await;
    let tick_frame = &seen[seen.find("event: lctick").unwrap()..];
    let data = tick_frame
        .split("data: ")
        .nth(1)
        .unwrap()
        .lines()
        .next()
        .unwrap();
    assert!(!data.trim().is_empty(), "{tick_frame}");
}

/// Plan E Task 3, the debt's closure: `#lc-flights` now lives exactly once
/// per shell page (`lc_room.html`, `lc_screen.html`) and zero times in
/// either fetched, per-viewer fragment (`lc_hand_pane`/`lc_mini_table`'s
/// output via the hand/table routes). A repaint that replaces one of those
/// fragments' markup — `lcApply`'s `pane.innerHTML`,
/// `lcApplyTable`'s `#lc-table.outerHTML` — can therefore no longer destroy
/// the flight layer or drop a flight's `onArrive`, because the layer is
/// never inside the swapped subtree.
#[tokio::test]
async fn test_the_flight_layer_lives_in_the_shells_not_the_fragments() {
    let app = test_app().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;
    post_form(&app, &alice, &format!("/room/{code}/lastcall/start"), "").await;

    let shell = body_string(get_shell(&app, &alice, &code).await).await;
    assert_eq!(
        shell.matches(r#"id="lc-flights""#).count(),
        1,
        "the phone shell must carry exactly one static flight layer: {shell}"
    );

    let screen = body_string(get(&app, &format!("/room/{code}/screen")).await).await;
    assert_eq!(
        screen.matches(r#"id="lc-flights""#).count(),
        1,
        "the big screen must carry exactly one static flight layer: {screen}"
    );

    let hand = body_string(get_hand(&app, &alice, &code).await).await;
    assert_eq!(
        hand.matches(r#"id="lc-flights""#).count(),
        0,
        "the fetched hand fragment must never contain the flight layer: {hand}"
    );

    let table = body_string(get_table(&app, &alice, &code).await).await;
    assert_eq!(
        table.matches(r#"id="lc-flights""#).count(),
        0,
        "the fetched table fragment must never contain the flight layer: {table}"
    );
}

/// Decision E12: a lagged `broadcast::Receiver` (one `Err(RecvError::Lagged)`
/// per gap, however many frames it covers — see `BroadcastStream`) now emits
/// a synthetic `lctick` with payload `"0"` instead of being silently dropped.
/// The channel holds 128 messages; `persist_and_broadcast_lc` publishes four
/// per handicap POST (game, room, lcpublic, lctick), so 40 POSTs (160 frames)
/// without draining the subscriber overruns it and forces a lag. Zero never
/// lowers the client's seq floor (the listener keeps `max(lcSeq, data)`) but
/// still fires the coalesced re-fetch, so this is content-filtered on the
/// synthetic payload itself, not just the event name.
#[tokio::test]
async fn test_a_lagged_subscriber_is_told_to_refetch() {
    let (app, pool) = test_app_with_pool().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;
    post_form(&app, &alice, &format!("/room/{code}/lastcall/start"), "").await;
    let alice_player = drinkinggame::db::get_player_by_name(&pool, "alice")
        .await
        .unwrap();

    let res = get(&app, &format!("/room/{code}/sse")).await;
    let mut body = res.into_body().into_data_stream();
    read_sse_until(&mut body, "event: lcpublic").await; // drain the snapshot

    // WITHOUT polling the body further: 40 POSTs x 4 frames each = 160,
    // which overruns the 128-capacity channel and lags this receiver.
    for _ in 0..40 {
        post_form(
            &app,
            &alice,
            &format!("/room/{code}/lastcall/handicap"),
            &format!("target={}&handicap_pct=150", alice_player.id),
        )
        .await;
    }

    let seen = read_sse_until(&mut body, "data: 0").await;
    assert!(seen.contains("event: lctick"), "{seen}");
}

// -------------------------------------------------------------
// Last Call (Task 3, fix-loop round 1): `lcApply`'s `innerHTML` swap on
// `[data-lc-pane="hand"]` was clobbering in-progress form state (the vessel
// container text, the focused handicap input, the deck <select>) on every
// broadcast from ANY player, including the caret position — reproduced in a
// live browser. Since this is client-side JS inside an Askama template,
// `node --check` never reaches it (it only walks `static/*.js` and
// `drinkinggame/assets/*.js` — CLAUDE.md), so this is a text-presence check
// on the rendered shell, not a behavioural one: it proves the
// capture-and-restore code shipped in the page a browser actually receives,
// not that a DOM diff/replay was run against it.
// -------------------------------------------------------------

#[tokio::test]
async fn test_lastcall_shell_ships_form_state_preservation() {
    let app = test_app().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;
    post_form(&app, &alice, &format!("/room/{code}/lastcall/start"), "").await;

    let html = body_string(get_shell(&app, &alice, &code).await).await;

    // Captures the focused element inside #lc-hand before the swap...
    assert!(html.contains("document.activeElement"), "{html}");
    // ...disambiguates which handicap row by its hidden `target` sibling...
    assert!(html.contains("elements.target"), "{html}");
    // ...guards the number-input selectionStart read/restore, which throws
    // in some browsers rather than just returning undefined...
    assert!(html.contains("selectionStart"), "{html}");
    assert!(html.contains("setSelectionRange"), "{html}");
    // ...restores the deck <select> even when it wasn't the focused
    // element (the "tabbed to container" case)...
    assert!(html.contains(r#"select[name="deck"]"#), "{html}");
    // ...and refocuses the restored element rather than leaving focus on
    // <body>, which is exactly what the live-browser repro observed.
    assert!(html.contains("el.focus("), "{html}");
}

// -------------------------------------------------------------
// Plan-end review fix wave (2026-08-09): findings I1 and M3.
// -------------------------------------------------------------

/// Plan-end review finding I1: pressing START must publish the phone GAME
/// tab, not just the room/public/tick messages. `LcPublic`/`LcTick` reach
/// only clients already on the Last Call shell, and at the instant START is
/// pressed nobody is there yet — without `broadcast_game` in
/// `persist_and_broadcast_lc`, starting Last Call was a complete visual
/// no-op on every phone, including the starter's, until a manual reload.
/// `render::lc_placeholder_panel` already renders "Last Call is running.
/// Open the table →"; this asserts that text actually reaches the `game`
/// SSE frame the moment the game starts.
#[tokio::test]
async fn test_lastcall_start_broadcasts_game_panel() {
    let app = test_app().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;

    // Subscribe before the game starts, and drain the four-frame idle
    // snapshot (leaderboard, game, screen, room), so the `game` frame this
    // test cares about is unambiguously the one START triggers.
    let res = get(&app, &format!("/room/{code}/sse")).await;
    let mut body = res.into_body().into_data_stream();
    read_sse_until(&mut body, "event: room").await;

    let res = post_form(&app, &alice, &format!("/room/{code}/lastcall/start"), "").await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    let seen = read_sse_until(&mut body, "event: game").await;
    let frame = &seen[seen.find("event: game").unwrap()..];
    assert!(
        frame.contains("Last Call is running"),
        "starting Last Call must broadcast the GAME panel so a phone still \
         sitting on the idle start cards sees the game begin without a \
         reload: {frame}"
    );
}

/// Plan-end review finding M3: the late-join `broadcast_lc` call in
/// `room_page`'s `last_call` block (`routes.rs`) had zero coverage —
/// `test_room_page_seats_late_joiner_in_lastcall` only asserts the DB row,
/// never subscribes to the stream, so deleting the broadcast, or moving it
/// outside the `lc_joined` guard or the lock, would ship silently green.
/// This subscribes as an already-seated player (bob) before cara's late
/// join and asserts bob's open stream actually receives the resulting
/// `lcpublic`/`lctick` pair.
#[tokio::test]
async fn test_room_page_late_join_broadcasts_lc() {
    let app = test_app().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let cara = login(&app, "cara", "1234").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;
    post_form(&app, &alice, &format!("/room/{code}/lastcall/start"), "").await;

    // bob is already seated and stays on the open SSE stream; only cara is
    // late-joining.
    let res = get(&app, &format!("/room/{code}/sse")).await;
    let mut body = res.into_body().into_data_stream();
    read_sse_until(&mut body, "event: lcpublic").await; // drain the snapshot

    let res = app
        .clone()
        .oneshot(
            Request::get(format!("/room/{code}"))
                .header(header::COOKIE, &cara)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    let seen = read_sse_until(&mut body, "event: lctick").await;
    assert!(
        seen.contains("event: lcpublic"),
        "cara's late join must broadcast the public panel to every phone \
         already on the stream: {seen}"
    );
}

#[test]
fn test_new_scene_roots_are_positioned() {
    // #lc-flights is position:absolute; inset:0; overflow:hidden. Without a
    // positioned ancestor it forms its containing block against the viewport
    // and clips every flight past the first screenful — nodes created with
    // correct deltas and never rendered. Invisible to any test that only
    // checks the flight lifecycle, which is why it is asserted here.
    //
    // Selectors are brace-anchored ("body.lc-screen {", not "body.lc-screen")
    // because split_once is substring matching: a bare ".lc-minitable" would
    // also match the prefix of ".lc-minitable-ring", silently inspecting the
    // wrong CSS block (the same latent flaw the brief's original ".lc-mini"
    // had against ".lc-mini-cost").
    let css = include_str!("../assets/lastcall.css");
    for root in ["body.lc-screen {", ".lc-minitable {"] {
        let block = css
            .split_once(root)
            .and_then(|(_, rest)| rest.split_once('}'))
            .map(|(b, _)| b)
            .unwrap_or_else(|| panic!("{root} not found in lastcall.css"));
        assert!(
            block.contains("position: relative"),
            "{root} must be positioned — it is a scene root for #lc-flights"
        );
    }
}

// -------------------------------------------------------------
// Last Call (Task 4): the big screen — lc_screen.html, the kind branch in
// screen_page, and the data-lc-live handoff between it and screen.html.
// -------------------------------------------------------------

/// The half of the handoff a code-read can't stand in for: a spectator
/// already parked on `screen.html` when Last Call starts must actually
/// receive `data-lc-live` on the wire, in the `screen` frame `broadcast_game`
/// publishes as part of `persist_and_broadcast_lc`. Mirrors
/// `test_lastcall_start_broadcasts_game_panel`, which proves the sibling
/// `game` frame on that same broadcast reaches an already-subscribed client
/// — this is the same call, checking the other frame it publishes.
#[tokio::test]
async fn test_lastcall_start_broadcasts_the_live_marker_on_the_screen_frame() {
    let app = test_app().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;

    // Subscribe before the game starts, and drain the four-frame idle
    // snapshot (leaderboard, game, screen, room), so the `screen` frame this
    // test cares about is unambiguously the one START triggers.
    let res = get(&app, &format!("/room/{code}/sse")).await;
    let mut body = res.into_body().into_data_stream();
    read_sse_until(&mut body, "event: room").await;

    let res = post_form(&app, &alice, &format!("/room/{code}/lastcall/start"), "").await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    // Publish order after START is game -> screen -> room -> lcpublic ->
    // lctick; `read_sse_until` returns only newly-read bytes, so this can't
    // be satisfied by the drained snapshot above.
    let seen = read_sse_until(&mut body, "event: screen").await;
    let frame = &seen[seen.find("event: screen").unwrap()..];
    assert!(
        frame.contains("data-lc-live"),
        "a spectator already on screen.html when Last Call starts must see \
         the live marker on the `screen` frame or it can never reload onto \
         the felt: {frame}"
    );
}

/// A real occurrence of the marker: whitespace, the literal, then an
/// immediate `>` with no `=` and no quote in between — the exact shape
/// `lc_screen_placeholder` emits (`<div class="screen-panel" data-lc-live>`)
/// and the only shape `document.querySelector("[data-lc-live]")` (what
/// `screen.html`/`lc_screen.html` now actually run against a parsed
/// fragment) would match as a real boolean attribute on a real element. Text
/// content and quoted attribute values — the only places `html_escape`d user
/// input can land — never take this shape.
fn contains_the_live_marker_as_a_real_attribute(html: &str) -> bool {
    html.contains(" data-lc-live>")
}

/// Fix round 1, Important 1: `auth::validate_name` accepts any non-empty
/// name up to 20 chars, no charset restriction, and `data-lc-live` (12
/// chars) is a legal name. `screen_panel_active`'s hero attributes the draw
/// to `html_escape(&c.drawer)` — none of `< > & "` appear in the literal, so
/// it survives escaping unchanged and lands in the `screen` frame as plain
/// text. Before this fix, `screen.html`/`lc_screen.html` substring-tested
/// the raw payload, so a player named exactly this string would trip the
/// Last Call handoff (or suppress it) on every Ring of Fire broadcast — a
/// display name bricking the room's big screen. This proves the attack
/// surface is real (the literal DOES reach the frame) and that it is not
/// mistaken for the marker (it never takes the real attribute's shape).
#[tokio::test]
async fn test_a_player_named_data_lc_live_does_not_masquerade_as_the_marker() {
    let app = test_app().await;
    let alice = login(&app, "alice", "1234").await;
    let mallory = login(&app, "data-lc-live", "6666").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &mallory, &code).await;
    post_form(
        &app,
        &alice,
        &format!("/room/{code}/game/start"),
        "preset_id=1",
    )
    .await;

    let res = get(&app, &format!("/room/{code}/sse")).await;
    let mut body = res.into_body().into_data_stream();
    read_sse_until(&mut body, "event: room").await; // drain the idle snapshot

    // mallory (name "data-lc-live") draws — her name becomes the hero's
    // drawer attribution, which flows into the `screen` frame.
    let res = post_form(&app, &mallory, &format!("/room/{code}/game/draw"), "").await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    let seen = read_sse_until(&mut body, "event: screen").await;
    let frame = &seen[seen.find("event: screen").unwrap()..];
    assert!(
        frame.contains("data-lc-live"),
        "test precondition: the player's name must actually reach the \
         screen frame, or this test proves nothing: {frame}"
    );
    assert!(
        !contains_the_live_marker_as_a_real_attribute(frame),
        "a display name must never render as the real data-lc-live boolean \
         attribute — a Ring of Fire spectator must not be sent into a \
         reload loop by a player's name: {frame}"
    );
}

#[tokio::test]
async fn test_screen_serves_the_last_call_felt_when_last_call_is_active() {
    let app = test_app().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;
    post_form(&app, &alice, &format!("/room/{code}/lastcall/start"), "").await;

    let res = get(&app, &format!("/room/{code}/screen")).await;
    assert_eq!(res.status(), StatusCode::OK);
    let html = body_string(res).await;
    assert!(html.contains(r#"<body class="lc-screen">"#), "{html}");
    assert_eq!(
        html.matches(r#"class="lc-seat""#).count(),
        2,
        "expected one seat per seated player: {html}"
    );
}

/// The regression this task's Class C exists for: two games that already
/// worked must render exactly what they rendered before the kind branch
/// landed. `screen_page`'s new `if` must fall through to the untouched path
/// for every kind but `last_call`.
#[tokio::test]
async fn test_screen_is_unchanged_for_ring_of_fire_and_three_man() {
    let app = test_app().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;

    // Idle: no game running yet.
    //
    // `data-lc-live>` (with the trailing `>`), not the bare substring
    // "data-lc-live": screen.html's own inline script now carries that
    // literal in its `e.data.includes("data-lc-live")` check on every
    // render, regardless of game kind, so a bare substring search would
    // always "find" it and the assertion would be meaningless. The marker
    // as an actual rendered boolean attribute (`lc_screen_placeholder`'s
    // output) always has a `>` immediately after it; the JS string literal
    // never does.
    let idle_html = body_string(get(&app, &format!("/room/{code}/screen")).await).await;
    assert!(
        idle_html.contains(r#"<body class="screen">"#),
        "{idle_html}"
    );
    assert!(idle_html.contains("screen-idle"), "{idle_html}");
    assert!(!idle_html.contains("data-lc-live>"), "{idle_html}");
    assert!(!idle_html.contains(r#"class="lc-screen""#), "{idle_html}");

    // Ring of Fire, running.
    let res = post_form(
        &app,
        &alice,
        &format!("/room/{code}/game/start"),
        "preset_id=1",
    )
    .await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    let rof_html = body_string(get(&app, &format!("/room/{code}/screen")).await).await;
    assert!(rof_html.contains(r#"<body class="screen">"#), "{rof_html}");
    assert!(rof_html.contains("52 of 52 left"), "{rof_html}");
    assert!(!rof_html.contains("data-lc-live>"), "{rof_html}");
    let res = post_form(&app, &alice, &format!("/room/{code}/game/end"), "").await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    // 3 Man, running.
    let res = post_form(&app, &alice, &format!("/room/{code}/tm/start"), "").await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    let tm_html = body_string(get(&app, &format!("/room/{code}/screen")).await).await;
    assert!(tm_html.contains(r#"<body class="screen">"#), "{tm_html}");
    assert!(tm_html.contains("3 MAN"), "{tm_html}");
    assert!(!tm_html.contains("data-lc-live>"), "{tm_html}");
}

/// A TV in the corner has no cookie. `get()` never attaches a Cookie header
/// at all — the spectator screen must still render.
#[tokio::test]
async fn test_the_last_call_screen_needs_no_session() {
    let app = test_app().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;
    post_form(&app, &alice, &format!("/room/{code}/lastcall/start"), "").await;

    let res = get(&app, &format!("/room/{code}/screen")).await;
    assert_eq!(res.status(), StatusCode::OK);
    let html = body_string(res).await;
    assert!(html.contains(r#"class="lc-seat""#), "{html}");
}

/// The handoff (`screen.html` <-> `lc_screen.html`) inverts the instant a
/// second screen-panel builder gains the `data-lc-live` marker. Direct
/// construction, not an HTTP round trip: this is a static property of the
/// builders themselves, independent of any particular room's state.
#[test]
fn test_exactly_one_screen_panel_builder_marks_itself_live() {
    let idle = drinkinggame::render::screen_panel_idle("QK4M");
    let active = drinkinggame::render::screen_panel_active(
        &drinkinggame::render::GameView {
            base_path: "",
            code: "QK4M",
            current: None,
            remaining: 52,
            held: vec![],
            counts: &[],
            announcement: None,
            anim_key: String::new(),
        },
        &[],
        0,
    );
    let over = drinkinggame::render::screen_panel_over(&drinkinggame::render::GameSummary {
        hardest: None,
        most_shots: None,
        room_total: 0,
        kings_cup: None,
        counts: vec![],
        house_rules: vec![],
    });
    let tm_over = drinkinggame::render::tm_screen_over(&drinkinggame::render::TmOverView {
        hardest: None,
        most_shots: None,
        room_total: 0,
    });
    let lc_live = drinkinggame::render::lc_screen_placeholder("QK4M");

    let marked: Vec<&str> = [
        ("screen_panel_idle", idle.as_str()),
        ("screen_panel_active", active.as_str()),
        ("screen_panel_over", over.as_str()),
        ("tm_screen_over", tm_over.as_str()),
        ("lc_screen_placeholder", lc_live.as_str()),
    ]
    .into_iter()
    .filter(|(_, html)| html.contains("data-lc-live"))
    .map(|(name, _)| name)
    .collect();

    assert_eq!(
        marked,
        vec!["lc_screen_placeholder"],
        "exactly one screen-panel builder may carry data-lc-live or the \
         screen.html <-> lc_screen.html handoff inverts"
    );
}

/// `lc_public_panel` now carries two `<template>` destinations, but it is
/// still ONE frame on ONE event — `broadcast_lc`'s two publishes, and the
/// SSE snapshot's chain, are unchanged. Filtered by event name, never
/// positionally: publish order is room -> lcpublic -> lctick, and the
/// connect-time snapshot is leaderboard -> game -> screen -> room ->
/// lcpublic, so counting `next()` calls would be fragile to how the
/// underlying stream happens to chunk bytes, not to a real frame count.
#[tokio::test]
async fn test_lcpublic_carries_both_templates_and_the_frame_count_is_unchanged() {
    use futures::StreamExt;
    let app = test_app().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;
    post_form(&app, &alice, &format!("/room/{code}/lastcall/start"), "").await;

    let res = get(&app, &format!("/room/{code}/sse")).await;
    let mut body = res.into_body().into_data_stream();
    let seen = read_sse_until(&mut body, "event: lcpublic").await;

    for name in ["leaderboard", "game", "screen", "room", "lcpublic"] {
        let marker = format!("event: {name}");
        assert_eq!(
            seen.matches(&marker).count(),
            1,
            "expected exactly one `{marker}` frame in the snapshot: {seen}"
        );
    }
    // The five per-name checks above only prove each of those five names
    // occurs exactly once — they don't bound the TOTAL. A sixth frame
    // carrying a brand-new event name, published anywhere before `lcpublic`
    // in the chain, would land inside `seen` and slip past every check above
    // silently. This is what the test's own name promises to catch.
    assert_eq!(
        seen.matches("event: ").count(),
        5,
        "connect-time snapshot for a Last Call room must stay five frames \
         total, no more: {seen}"
    );

    let frame = &seen[seen.find("event: lcpublic").unwrap()..];
    assert!(frame.contains("data-lc-banner"), "{frame}");
    assert!(frame.contains("data-lc-screen"), "{frame}");

    // Bounded wait: nothing more should arrive unprompted. A sixth frame
    // here would mean an extra publish snuck into the snapshot chain.
    match tokio::time::timeout(std::time::Duration::from_millis(200), body.next()).await {
        Err(_) => {}   // timed out waiting: correct, nothing more was sent.
        Ok(None) => {} // stream ended: also fine.
        Ok(Some(chunk)) => {
            let extra = String::from_utf8(chunk.unwrap().to_vec()).unwrap();
            panic!("unexpected extra frame after the five-frame snapshot: {extra}");
        }
    }
}

// -------------------------------------------------------------
// Last Call (Task 5): the phone TABLE tab — `GET /room/{code}/lastcall/table`.
// Per-viewer data (D.2's bottom-centre rotation), fetched rather than
// broadcast, following `lc_hand_handler`'s exact shape: no player
// identifier of any kind, identity from the session cookie alone.
// -------------------------------------------------------------

async fn get_table(app: &Router, cookie: &str, code: &str) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::get(format!("/room/{code}/lastcall/table"))
                .header(header::COOKIE, cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
}

/// Pulls the `data-seq` off the `#lc-table` root, so tests can assert two
/// fetches of the same state agree on freshness without hardcoding it.
fn table_seq(html: &str) -> u64 {
    let marker = "id=\"lc-table\" data-seq=\"";
    let start = html.find(marker).unwrap() + marker.len();
    let rest = &html[start..];
    rest[..rest.find('"').unwrap()].parse().unwrap()
}

/// The core claim (D.2, and the reason this route is a fetch and not a
/// broadcast): same room, same state, same seq — different HTML, each with
/// the requester's own seat in the bottom slot.
#[tokio::test]
async fn test_two_players_get_different_rotations_of_the_same_table() {
    let (app, pool) = test_app_with_pool().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let cara = login(&app, "cara", "1234").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;
    room_page_html(&app, &cara, &code).await;
    post_form(&app, &alice, &format!("/room/{code}/lastcall/start"), "").await;

    let st = lc_state(&pool, &code).await;
    let alice_player = drinkinggame::db::get_player_by_name(&pool, "alice")
        .await
        .unwrap();
    let bob_player = drinkinggame::db::get_player_by_name(&pool, "bob")
        .await
        .unwrap();
    let alice_seat = st.seat_of(alice_player.id).unwrap();
    let bob_seat = st.seat_of(bob_player.id).unwrap();
    assert_ne!(alice_seat, bob_seat, "test precondition: distinct seats");

    let alice_table = body_string(get_table(&app, &alice, &code).await).await;
    let bob_table = body_string(get_table(&app, &bob, &code).await).await;
    assert_ne!(alice_table, bob_table);

    // Neither request changed any state, so both reads land at the same seq.
    assert_eq!(table_seq(&alice_table), table_seq(&bob_table));

    let bottom = drinkinggame::lc_layout::seat_positions(3)[0];
    assert!(
        alice_table.contains(&format!(
            r#"style="left:{}%;top:{}%" data-seat="{alice_seat}""#,
            bottom.0, bottom.1
        )),
        "alice's own seat ({alice_seat}) should hold the bottom slot in her \
         view: {alice_table}"
    );
    assert!(
        bob_table.contains(&format!(
            r#"style="left:{}%;top:{}%" data-seat="{bob_seat}""#,
            bottom.0, bottom.1
        )),
        "bob's own seat ({bob_seat}) should hold the bottom slot in his \
         view: {bob_table}"
    );
}

/// Asserts the §6.1 constraint behaviourally, not just by signature:
/// appending a caller-supplied player identifier to the query string must
/// change nothing about the response — mirrors
/// `test_lastcall_hand_route_takes_no_player_input`.
#[tokio::test]
async fn test_the_table_route_takes_no_player_identifier() {
    let (app, pool) = test_app_with_pool().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;
    post_form(&app, &alice, &format!("/room/{code}/lastcall/start"), "").await;
    let bob_player = drinkinggame::db::get_player_by_name(&pool, "bob")
        .await
        .unwrap();

    let baseline = body_string(get_table(&app, &alice, &code).await).await;

    let with_player_id = body_string(
        app.clone()
            .oneshot(
                Request::get(format!(
                    "/room/{code}/lastcall/table?player_id={}",
                    bob_player.id
                ))
                .header(header::COOKIE, &alice)
                .body(Body::empty())
                .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(baseline, with_player_id);

    let with_target = body_string(
        app.oneshot(
            Request::get(format!(
                "/room/{code}/lastcall/table?target={}",
                bob_player.id
            ))
            .header(header::COOKIE, &alice)
            .body(Body::empty())
            .unwrap(),
        )
        .await
        .unwrap(),
    )
    .await;
    assert_eq!(baseline, with_target);
}

#[tokio::test]
async fn test_the_table_route_requires_a_session() {
    let app = test_app().await;
    let alice = login(&app, "alice", "1234").await;
    let code = create_room(&app, &alice).await;

    let res = app
        .oneshot(
            Request::get(format!("/room/{code}/lastcall/table"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(
        res.status().is_redirection() || res.status() == StatusCode::UNAUTHORIZED,
        "expected a redirect or 401, got {}",
        res.status()
    );
    assert!(body_string(res).await.is_empty());
}

/// A room member the app has no route for reaching un-seated: every real
/// HTTP path into a room (`/room/{code}`) auto-seats a member into
/// `LastCallState` the instant a Last Call game is active, so this test
/// puts cara in `room_members` directly at the DB layer — the same
/// mid-game-join, no-vessel-yet state the brief describes — and asserts the
/// route falls back to the unrotated table rather than panicking on a
/// missing seat.
#[tokio::test]
async fn test_an_unseated_member_gets_the_unrotated_table() {
    let (app, pool) = test_app_with_pool().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let cara = login(&app, "cara", "1234").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;
    post_form(&app, &alice, &format!("/room/{code}/lastcall/start"), "").await;

    let room = drinkinggame::db::get_open_room(&pool, &code).await.unwrap();
    let cara_player = drinkinggame::db::get_player_by_name(&pool, "cara")
        .await
        .unwrap();
    drinkinggame::db::join_room(&pool, room.id, cara_player.id).await;

    let st = lc_state(&pool, &code).await;
    assert!(
        st.seat_of(cara_player.id).is_none(),
        "test precondition: cara must be unseated"
    );

    let table = body_string(get_table(&app, &cara, &code).await).await;
    assert!(!table.contains("data-me"));
    let expected = format!(
        r#"<div id="lc-table" data-seq="{}">{}</div>"#,
        st.public_view().seq,
        drinkinggame::lc_render::lc_mini_table(&st.public_view(), None),
    );
    assert_eq!(table, expected);
}

/// Both panes (HAND and TABLE) are always in the DOM, one just `hidden` —
/// so the page must not carry two `#lc-flights` or `#lc-felt` roots, and no
/// `data-flight-anchor` may repeat across the two surfaces: `lcAnchor`
/// returns the first match, so a duplicate would silently misroute a
/// flight. Mirrors `lc_render`'s own
/// `test_no_duplicate_anchors_or_ids_on_either_surface`, at the whole-page
/// level.
#[tokio::test]
async fn test_one_flight_layer_per_page() {
    let app = test_app().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;
    post_form(&app, &alice, &format!("/room/{code}/lastcall/start"), "").await;

    let html = body_string(get_shell(&app, &alice, &code).await).await;
    for needle in ["id=\"lc-felt\"", "id=\"lc-flights\""] {
        assert_eq!(
            html.matches(needle).count(),
            1,
            "duplicate {needle}: {html}"
        );
    }
    let mut anchors: Vec<&str> = html
        .match_indices("data-flight-anchor=\"")
        .map(|(i, m)| {
            let rest = &html[i + m.len()..];
            &rest[..rest.find('"').unwrap()]
        })
        .collect();
    let before = anchors.len();
    anchors.sort_unstable();
    anchors.dedup();
    assert_eq!(anchors.len(), before, "duplicate flight anchor: {html}");
}

// -------------------------------------------------------------
// Last Call (Task 6): the seat ceiling on both seating paths, and
// `POST /room/{code}/lastcall/end` — a game can end without ending the
// room. `add_player`'s signature change and `LastCallState::new`'s cap are
// covered at the unit level in `last_call.rs`; these integration tests
// cover route-level effects: room survival, the big-screen handoff run in
// reverse, membership, and a full table's ninth visitor.
// -------------------------------------------------------------

#[tokio::test]
async fn test_ending_last_call_keeps_the_room_open() {
    let (app, pool) = test_app_with_pool().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;
    post_form(&app, &alice, &format!("/room/{code}/lastcall/start"), "").await;

    let res = post_form(&app, &alice, &format!("/room/{code}/lastcall/end"), "").await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    // The room survives, the game does not.
    let room = drinkinggame::db::get_open_room(&pool, &code)
        .await
        .expect("room must still be open");
    assert!(drinkinggame::db::get_active_game(&pool, room.id)
        .await
        .is_none());

    // And the start card comes back: a plain GET no longer redirects into
    // the Last Call shell (no active game left to redirect to), it renders
    // the generic room page with the idle panel's three start cards.
    let html = room_page_html(&app, &alice, &code).await;
    assert!(html.contains(r#"class="game-idle""#), "{html}");
    assert!(
        html.contains(r#"<h2 class="start-title">Last Call</h2>"#),
        "{html}"
    );
}

/// Filter by event name, not position — the fix Task 4 needed for its own
/// positionally-indexed SSE test applies here too: `lc_end_handler`
/// publishes `game`, then `screen`, then `broadcast_room`'s `room`, three
/// frames not one.
#[tokio::test]
async fn test_ending_publishes_an_unmarked_screen_frame() {
    let app = test_app().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;
    post_form(&app, &alice, &format!("/room/{code}/lastcall/start"), "").await;

    let res = get(&app, &format!("/room/{code}/sse")).await;
    let mut body = res.into_body().into_data_stream();
    read_sse_until(&mut body, "event: room").await; // drain the start snapshot

    let end_res = post_form(&app, &alice, &format!("/room/{code}/lastcall/end"), "").await;
    assert_eq!(end_res.status(), StatusCode::NO_CONTENT);

    let seen = read_sse_until(&mut body, "event: screen").await;
    let frame = &seen[seen.find("event: screen").unwrap()..];
    assert!(
        !contains_the_live_marker_as_a_real_attribute(frame),
        "the screen frame published on game end must not carry data-lc-live \
         or every open lc_screen.html can never fall back to the generic \
         screen: {frame}"
    );
}

/// The mirror of the test above: while Last Call is actually running, the
/// `game` frame must NOT match `.game-idle`, or a future change to
/// `lc_placeholder_panel` (or to `game_idle_panel`) could silently kick
/// every phone off the table mid-round via the new listener in
/// `lc_room.html`. Checked on the snapshot a fresh subscriber receives at
/// connect — the shape most likely to regress unnoticed, since it fires on
/// every page load, not just on END GAME.
#[tokio::test]
async fn test_lastcall_live_game_frame_does_not_carry_game_idle() {
    let app = test_app().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;
    post_form(&app, &alice, &format!("/room/{code}/lastcall/start"), "").await;

    let res = get(&app, &format!("/room/{code}/sse")).await;
    let mut body = res.into_body().into_data_stream();
    // Read until the game frame arrives rather than counting out four of
    // them. A fixed count is wrong in both directions: if the server ever
    // coalesces frames into one chunk the loop blocks forever waiting for a
    // fourth that never comes, and if it splits one the assertion silently
    // starts inspecting a different frame than it names. `read_sse_until`
    // times out with a clear message instead, which also subsumes the old
    // `seen_game` flag.
    let seen = read_sse_until(&mut body, "event: game").await;
    let rest = &seen[seen.find("event: game").unwrap()..];
    // Cut at the frame boundary so a coalesced chunk cannot smuggle a later
    // frame into the assertion.
    let frame = rest.split("\n\n").next().unwrap_or(rest);
    // Same selector-shaped check the positive test below uses: a bare
    // `class="game-idle"` needle would miss a multi-class root that the
    // browser's querySelector still matches.
    assert!(!matches_the_game_idle_selector(frame), "{frame}");
}

/// True when the payload holds an element `querySelector(".game-idle")`
/// would match: `game-idle` as a whitespace-delimited token of some `class`
/// attribute, rather than the literal substring `class="game-idle"`. A root
/// that gained a second class would still match in the browser but slip
/// past the substring form, so the byte-shape proxy has to model the
/// selector, not the current spelling of the markup.
fn matches_the_game_idle_selector(payload: &str) -> bool {
    payload.match_indices("class=\"").any(|(i, m)| {
        let rest = &payload[i + m.len()..];
        rest.find('"')
            .is_some_and(|end| rest[..end].split_whitespace().any(|c| c == "game-idle"))
    })
}

/// The other half of the handoff, and the one nothing asserted: ending the
/// game must publish a `game` frame the phone's listener will actually act
/// on. `lc_room.html` redirects on `querySelector(".game-idle")`, so the
/// coupling between this handler's choice of builder and that selector is
/// the whole exit path — swap `idle_panel` for any other builder and every
/// other test here still passes while every phone sits on a dead table
/// watching a game that ended.
///
/// Its mirror above proves a live game publishes no match; this proves an
/// ended one does. Neither alone pins the contract.
#[tokio::test]
async fn test_ending_publishes_a_game_frame_the_phone_acts_on() {
    let app = test_app().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;
    post_form(&app, &alice, &format!("/room/{code}/lastcall/start"), "").await;

    let res = get(&app, &format!("/room/{code}/sse")).await;
    let mut body = res.into_body().into_data_stream();
    read_sse_until(&mut body, "event: room").await; // drain the start snapshot

    let end_res = post_form(&app, &alice, &format!("/room/{code}/lastcall/end"), "").await;
    assert_eq!(end_res.status(), StatusCode::NO_CONTENT);

    // `lc_end_handler` publishes game, then screen, then broadcast_room's
    // room — filter by event name, never by position.
    let seen = read_sse_until(&mut body, "event: game").await;
    let frame = &seen[seen.find("event: game").unwrap()..];
    assert!(
        matches_the_game_idle_selector(frame),
        "the game frame published on end must match the phone's \
         .game-idle selector or no phone ever leaves the table: {frame}"
    );
}

/// No JS test harness exists in this repo (recorded on Task 4's ledger
/// entry) — this is the same Rust-side byte-shape proxy that fix round
/// used there: the shell wires the `game` SSE event to a parsed-and-selected
/// redirect, never a raw substring test against the user-controllable
/// `e.data` payload (a player's display name rides in the very same frame).
#[tokio::test]
async fn test_lastcall_shell_wires_the_game_idle_redirect_safely() {
    let app = test_app().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;
    post_form(&app, &alice, &format!("/room/{code}/lastcall/start"), "").await;

    let html = body_string(get_shell(&app, &alice, &code).await).await;
    assert!(html.contains(r#"addEventListener("game""#), "{html}");
    assert!(html.contains(r#"querySelector(".game-idle")"#), "{html}");
    assert!(!html.contains(r#"e.data.includes("game-idle")"#), "{html}");
    assert!(!html.contains(r#".includes("game-idle")"#), "{html}");
}

#[tokio::test]
async fn test_ending_requires_membership() {
    let app = test_app().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let cara = login(&app, "cara", "1234").await; // never joins the room
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;
    post_form(&app, &alice, &format!("/room/{code}/lastcall/start"), "").await;

    let res = post_form(&app, &cara, &format!("/room/{code}/lastcall/end"), "").await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // No cookie at all: the same PlayerSession redirect the crate already
    // produces for any other room route.
    let res = app
        .oneshot(
            Request::post(format!("/room/{code}/lastcall/end"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
}

/// Unseated, but not locked out: the ROOM tab (HAND/TABLE/LOG shell, no
/// separate ROOM tab on Last Call) and the TABLE tab both render, and the
/// table renders unrotated — spec §6.1/D.2's contract that a member who
/// missed a seat still reaches the room, just with `me = None`.
#[tokio::test]
async fn test_a_ninth_member_can_still_open_the_room() {
    let (app, pool) = test_app_with_pool().await;
    let alice = login(&app, "alice", "1234").await;
    let code = create_room(&app, &alice).await;

    // Seven more, eight total, filling the table to MAX_SEATS.
    for i in 1..=7 {
        let cookie = login(&app, &format!("p{i}"), "1234").await;
        room_page_html(&app, &cookie, &code).await;
    }
    post_form(&app, &alice, &format!("/room/{code}/lastcall/start"), "").await;

    let before = lc_state(&pool, &code).await;
    assert_eq!(before.players.len(), drinkinggame::last_call::MAX_SEATS);

    // The ninth visitor opens the room link.
    let ninth = login(&app, "ninth", "1234").await;
    let res = app
        .clone()
        .oneshot(
            Request::get(format!("/room/{code}"))
                .header(header::COOKIE, &ninth)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        StatusCode::SEE_OTHER,
        "still joins the room and is redirected into the lc shell"
    );

    let ninth_player = drinkinggame::db::get_player_by_name(&pool, "ninth")
        .await
        .unwrap();
    let room = drinkinggame::db::get_open_room(&pool, &code).await.unwrap();
    assert!(
        drinkinggame::db::is_room_member(&pool, room.id, ninth_player.id).await,
        "not seated is not the same as not a member"
    );

    // Re-read the state AFTER the join, not before. Asserting
    // `seat_of(ninth) == None` against `before` proves nothing: that snapshot
    // was taken while the ninth player row did not yet exist, so its id could
    // not appear in it whether the ceiling held or not. The live claim is
    // that the mid-game join hook ran, added the member, and declined to seat
    // them — which only a post-join read can show.
    let after = lc_state(&pool, &code).await;
    assert_eq!(
        after.players.len(),
        drinkinggame::last_call::MAX_SEATS,
        "the join hook must not grow the table past the ceiling"
    );
    assert!(
        after.seat_of(ninth_player.id).is_none(),
        "a member who arrived at a full table holds no seat"
    );

    // The shell itself renders, and the table renders unrotated: a seated
    // viewer's fragment always carries exactly one `data-me`
    // (lc_render.rs's own test), so a ninth member's fragment carrying none
    // is the same "unrotated" proof at the whole-page level.
    let shell = body_string(get_shell(&app, &ninth, &code).await).await;
    assert!(
        !shell.contains("data-me"),
        "an unseated member must render the unrotated table: {shell}"
    );
}

/// `lc_room.html`'s new `game` listener redirects to `/room/{code}` the
/// instant a `game` frame's payload matches `.game-idle`, and it does so
/// unconditionally — there is no room-scoped check. `POST /room/{code}/end`
/// (a different route from `/lastcall/end`, ends the whole night) only ever
/// publishes `RoomMessage::Ended`, never `RoomMessage::Game`
/// (`end_room_handler`, `routes.rs`) — but that is a property of a route
/// this task did not touch, so it is worth pinning with a test rather than
/// trusting the read: if `Ended` ever raced a `game-idle` `Game` frame
/// published first, every phone on the Last Call shell would navigate to
/// `/room/{code}` instead of `/`, landing on "Room not found" once the room
/// row is actually gone.
#[tokio::test]
async fn test_ending_the_room_never_races_the_game_idle_redirect() {
    let app = test_app().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;
    post_form(&app, &alice, &format!("/room/{code}/lastcall/start"), "").await;

    let res = get(&app, &format!("/room/{code}/sse")).await;
    let mut body = res.into_body().into_data_stream();
    read_sse_until(&mut body, "event: room").await; // drain the start snapshot

    let end_res = post_form(&app, &alice, &format!("/room/{code}/end"), "").await;
    assert_eq!(end_res.status(), StatusCode::SEE_OTHER);

    let seen = read_sse_until(&mut body, "event: ended").await;
    let before_ended = &seen[..seen.find("event: ended").unwrap()];
    assert!(
        !matches_the_game_idle_selector(before_ended),
        "a game-idle Game frame published before Ended would send every LC \
         phone to /room/{{code}} instead of /, right as the room disappears: \
         {seen}"
    );
}

// -------------------------------------------------------------
// Last Call (Plan E Task 1): the beat-loop action routes — arm, disarm,
// target, lock, draw. All five sit behind `lc_lock` -> `load_lc`'s
// member/active-game/kind gate (Task 1's own guard chain, unchanged);
// what's new here is the per-route broadcast policy (decision E5: tick-only
// for arm/disarm/target, full publish for lock/draw) and the RNG-in-the-
// route draw path.
// -------------------------------------------------------------

use drinkinggame::last_call::{Beat, Deck};

async fn lc_game_id(pool: &sqlx::SqlitePool, code: &str) -> i64 {
    let room = drinkinggame::db::get_open_room(pool, code).await.unwrap();
    drinkinggame::db::get_active_game(pool, room.id)
        .await
        .unwrap()
        .id
}

/// Pulls the `data-seq` off the `#lc-hand` root — the private twin of
/// `table_seq` above, used where a test cares that an action bumped `seq`
/// but has no other observable effect yet (Task 4 renders the rest).
fn hand_seq(html: &str) -> u64 {
    let marker = "id=\"lc-hand\" data-seq=\"";
    let start = html.find(marker).unwrap() + marker.len();
    let rest = &html[start..];
    rest[..rest.find('"').unwrap()].parse().unwrap()
}

/// Two real sessions, a real started game, then the state hand-rebuilt with
/// the real player ids at `Beat::Lock` — `set_vessel`'s F6 opener gives each
/// seat a real hand to arm from without dealing with the Draw beat's own
/// gating in every test. Returns `(app, pool, code, alice, bob, alice_id,
/// bob_id)`; alice is seat 0 (Beer), bob is seat 1 (Cider).
async fn lc_action_rig() -> (Router, sqlx::SqlitePool, String, String, String, i64, i64) {
    let (app, pool) = test_app_with_pool().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;
    post_form(&app, &alice, &format!("/room/{code}/lastcall/start"), "").await;

    let alice_id = drinkinggame::db::get_player_by_name(&pool, "alice")
        .await
        .unwrap()
        .id;
    let bob_id = drinkinggame::db::get_player_by_name(&pool, "bob")
        .await
        .unwrap()
        .id;

    let mut st = LastCallState::new(vec![(alice_id, "alice".into()), (bob_id, "bob".into())], 7);
    st.set_vessel(alice_id, Deck::Beer, "can").unwrap();
    st.set_vessel(bob_id, Deck::Cider, "bottle").unwrap();
    st.beat = Beat::Lock;
    let game_id = lc_game_id(&pool, &code).await;
    drinkinggame::db::set_game_state(&pool, game_id, &st.to_json()).await;

    (app, pool, code, alice, bob, alice_id, bob_id)
}

/// Decision E5: arm changes nothing legible on any public surface (the
/// public projection never reads `armed` — only `locked`), so it publishes
/// `LcTick` alone. The frame between the POST and the tick must carry no
/// `lcpublic`/`game`/`room` — a full publish would be free-riding
/// information a spectator has no business seeing yet.
#[tokio::test]
async fn test_lc_arm_is_a_tick_only_broadcast() {
    let (app, _pool, code, alice, bob, _alice_id, _bob_id) = lc_action_rig().await;

    let res = get(&app, &format!("/room/{code}/sse")).await;
    let mut body = res.into_body().into_data_stream();
    read_sse_until(&mut body, "event: lcpublic").await; // drain the snapshot

    let res = post_form(
        &app,
        &alice,
        &format!("/room/{code}/lastcall/arm"),
        "card_id=beer-01",
    )
    .await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    let seen = read_sse_until(&mut body, "event: lctick").await;
    assert!(
        !seen.contains("event: lcpublic")
            && !seen.contains("event: game")
            && !seen.contains("event: room"),
        "{seen}"
    );

    let alice_hand = body_string(get_hand(&app, &alice, &code).await).await;
    assert!(alice_hand.contains("ARMED 1"), "{alice_hand}");
    assert!(alice_hand.contains("beer-01"), "{alice_hand}");

    let bob_hand = body_string(get_hand(&app, &bob, &code).await).await;
    assert!(bob_hand.contains("ARMED 0"), "{bob_hand}");
    assert!(!bob_hand.contains("beer-01"), "{bob_hand}");

    // disarm is arm's mirror: same tick-only policy, same guard chain
    // (route-registration typos — wiring `/disarm` to `lc_arm_handler` —
    // are exactly what a dedicated assertion here catches).
    let res = post_form(
        &app,
        &alice,
        &format!("/room/{code}/lastcall/disarm"),
        "card_id=beer-01",
    )
    .await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    let seen = read_sse_until(&mut body, "event: lctick").await;
    assert!(
        !seen.contains("event: lcpublic")
            && !seen.contains("event: game")
            && !seen.contains("event: room"),
        "{seen}"
    );

    let alice_hand = body_string(get_hand(&app, &alice, &code).await).await;
    assert!(alice_hand.contains("ARMED 0"), "{alice_hand}");
    assert!(alice_hand.contains("beer-01"), "{alice_hand}"); // back in the wheel
}

/// `lock` is public — the seat's lock tick is legible on the mini table and
/// the big screen — but §3.4.1 still holds: the frame carries the marker,
/// never the card that got locked.
#[tokio::test]
async fn test_lc_lock_publishes_the_tick_not_the_cards() {
    let (app, _pool, code, alice, _bob, _alice_id, _bob_id) = lc_action_rig().await;

    post_form(
        &app,
        &alice,
        &format!("/room/{code}/lastcall/arm"),
        "card_id=beer-01",
    )
    .await;
    let res = post_form(
        &app,
        &alice,
        &format!("/room/{code}/lastcall/target"),
        "card_id=beer-01&target=1", // bob's seat
    )
    .await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    let res = get(&app, &format!("/room/{code}/sse")).await;
    let mut body = res.into_body().into_data_stream();
    read_sse_until(&mut body, "event: lcpublic").await; // drain the snapshot

    let res = post_form(&app, &alice, &format!("/room/{code}/lastcall/lock"), "").await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    let seen = read_sse_until(&mut body, "event: lcpublic").await;
    // The screen plaque's public lock marker (`lc_render::player_plaque`) is
    // an `is-locked` class plus a `lc-lock-tick` glyph, not a `data-locked`
    // attribute — that attribute belongs to the mini table
    // (`lc_render::lc_mini_table`) and the private armed column
    // (`lc_render::armed_column`), neither of which this frame carries.
    assert!(seen.contains("is-locked"), "{seen}");
    assert!(seen.contains("lc-lock-tick"), "{seen}");
    assert!(!seen.contains("beer-01"), "{seen}");
    assert!(!seen.contains("Nudge"), "{seen}"); // beer-01's title
}

/// DDv2 6.3: the two named-card refusals carry the card id in a plain-text
/// 422 body the action bar shows verbatim, rather than collapsing into
/// `map_lc`'s bare-422 wildcard. beer-01 targets "one" — locking it unarmed
/// of a target trips `LcError::NeedsTarget` in `lock_in`.
#[tokio::test]
async fn test_lc_lock_needs_target_names_the_card() {
    let (app, _pool, code, alice, _bob, _alice_id, _bob_id) = lc_action_rig().await;

    post_form(
        &app,
        &alice,
        &format!("/room/{code}/lastcall/arm"),
        "card_id=beer-01",
    )
    .await;

    let res = post_form(&app, &alice, &format!("/room/{code}/lastcall/lock"), "").await;
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body = body_string(res).await;
    assert!(body.contains("beer-01"), "{body}");
    assert!(body.contains("needs a target"), "{body}");
}

/// The guard chain shared by every action route: non-member -> 403, the
/// other game kind -> 409 (`WrongGameKind`), no active game at all -> 404
/// (`NoActiveGame`) — all three from `load_lc`, none of them route-specific.
#[tokio::test]
async fn test_lc_action_routes_are_guarded() {
    let app = test_app().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let carol = login(&app, "carol", "1111").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;
    post_form(&app, &alice, &format!("/room/{code}/lastcall/start"), "").await;

    // carol never opened the room: not a member.
    let res = post_form(
        &app,
        &carol,
        &format!("/room/{code}/lastcall/arm"),
        "card_id=beer-01",
    )
    .await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    post_form(&app, &alice, &format!("/room/{code}/lastcall/end"), "").await;

    // A 3 Man game active in the same room: WrongGameKind.
    post_form(&app, &alice, &format!("/room/{code}/tm/start"), "").await;
    let res = post_form(&app, &alice, &format!("/room/{code}/lastcall/lock"), "").await;
    assert_eq!(res.status(), StatusCode::CONFLICT);
    post_form(&app, &alice, &format!("/room/{code}/tm/end"), "").await;

    // No active game at all.
    let res = post_form(
        &app,
        &alice,
        &format!("/room/{code}/lastcall/draw"),
        "vessel=0",
    )
    .await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

/// The one route with RNG. `finish_and_draw`'s `expected` count is
/// `min(DRAW_PER_VESSEL, shoe count)` (D7), so a shoe holding at least 5
/// always deals exactly 5. Asserted at the state level (not just the
/// fragment's `data-count`): the five newly-dealt cards are all `Deck::Beer`
/// — proof identity selection stayed scoped to the vessel drawn from — and
/// the Beer shoe count actually debited by 5 (36 -> 31), proof the draw
/// isn't just publishing a frame with a stale count.
#[tokio::test]
async fn test_lc_draw_deals_five_from_the_vessels_deck() {
    let (app, pool) = test_app_with_pool().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;
    post_form(&app, &alice, &format!("/room/{code}/lastcall/start"), "").await;

    let alice_id = drinkinggame::db::get_player_by_name(&pool, "alice")
        .await
        .unwrap()
        .id;
    let bob_id = drinkinggame::db::get_player_by_name(&pool, "bob")
        .await
        .unwrap()
        .id;

    let mut st = LastCallState::new(vec![(alice_id, "alice".into()), (bob_id, "bob".into())], 7);
    st.set_vessel(alice_id, Deck::Beer, "can").unwrap();
    st.set_vessel(bob_id, Deck::Cider, "bottle").unwrap();
    st.round = 2;
    st.players[0].hand.truncate(4); // 4 cards in hand before the draw
    for entry in st.deck_counts.iter_mut() {
        if entry.0 == Deck::Beer {
            entry.1 = 36;
        }
    }
    let game_id = lc_game_id(&pool, &code).await;
    drinkinggame::db::set_game_state(&pool, game_id, &st.to_json()).await;

    let res = get(&app, &format!("/room/{code}/sse")).await;
    let mut body = res.into_body().into_data_stream();
    read_sse_until(&mut body, "event: lcpublic").await; // drain the snapshot

    let res = post_form(
        &app,
        &alice,
        &format!("/room/{code}/lastcall/draw"),
        "vessel=0",
    )
    .await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    // Deck counts moved (public), so the FULL publish ran, not a bare tick.
    let seen = read_sse_until(&mut body, "event: lcpublic").await;
    assert!(seen.contains("data-lc-public"), "{seen}");

    let hand_html = body_string(get_hand(&app, &alice, &code).await).await;
    assert!(hand_html.contains(r#"data-count="9""#), "{hand_html}");

    let after = lc_state(&pool, &code).await;
    let alice_seat = after.seat_of(alice_id).unwrap();
    assert_eq!(after.players[alice_seat].hand.len(), 9);
    assert!(
        after.players[alice_seat].hand[4..]
            .iter()
            .all(|c| c.deck == Deck::Beer),
        "the 5 newly-drawn cards must all come from the Beer shoe: {:?}",
        after.players[alice_seat].hand
    );
    let beer_left = after
        .deck_counts
        .iter()
        .find(|(d, _)| *d == Deck::Beer)
        .unwrap()
        .1;
    assert_eq!(beer_left, 31, "the Beer shoe must be debited by exactly 5");

    // Second draw the same round: TBD-5, one per player per round.
    let res = post_form(
        &app,
        &alice,
        &format!("/room/{code}/lastcall/draw"),
        "vessel=0",
    )
    .await;
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

/// `target=""` clears/omits a target (self/all/table cards, or unsetting a
/// one-target card); any other value must parse as a seat index or the
/// route bails before the engine ever sees it. The successful `target=1`
/// case is asserted by its `seq` bump — Task 4 renders the selected option.
#[tokio::test]
async fn test_lc_target_accepts_empty_as_none() {
    let (app, _pool, code, alice, _bob, _alice_id, _bob_id) = lc_action_rig().await;

    // beer-03 targets "self".
    post_form(
        &app,
        &alice,
        &format!("/room/{code}/lastcall/arm"),
        "card_id=beer-03",
    )
    .await;
    let res = post_form(
        &app,
        &alice,
        &format!("/room/{code}/lastcall/target"),
        "card_id=beer-03&target=",
    )
    .await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    let res = post_form(
        &app,
        &alice,
        &format!("/room/{code}/lastcall/target"),
        "card_id=beer-03&target=abc",
    )
    .await;
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let before_seq = hand_seq(&body_string(get_hand(&app, &alice, &code).await).await);

    post_form(
        &app,
        &alice,
        &format!("/room/{code}/lastcall/arm"),
        "card_id=beer-01",
    )
    .await;
    let res = post_form(
        &app,
        &alice,
        &format!("/room/{code}/lastcall/target"),
        "card_id=beer-01&target=1", // bob's seat
    )
    .await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    let after_seq = hand_seq(&body_string(get_hand(&app, &alice, &code).await).await);
    assert!(
        after_seq > before_seq,
        "before={before_seq} after={after_seq}"
    );
}

// -------------------------------------------------------------
// Last Call (Plan E Task 2): the beat clock — the persisted deadline field,
// the auto-beat advance chain, the 1 Hz ticker riding mechanics::spawn_ticker
// (already spawned by router_with_pool, so it runs inside every test app —
// subscribing the SSE stream is what puts a room in active_rooms()), the
// begin route, and lock's all-locked early advance.
// -------------------------------------------------------------

fn unix_ms_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

/// The ticker's advisory-read-then-relock-then-recheck path only fires
/// something when it finds an EXPIRED deadline on a hub-active room: rig at
/// `Beat::Lock` (via `lc_action_rig`), stamp an already-past
/// `beat_deadline_ms` directly onto the persisted blob (no route call —
/// nothing else has set this deadline yet, Task 1's rig doesn't arm the
/// clock), then subscribe SSE and wait for the ticker's own 1 Hz sweep to
/// pick it up. The `reveal` marker alone would also match the room's initial
/// SSE snapshot if that snapshot happened to say `reveal` (it can't here,
/// since it's taken before the tick fires, but nothing about the marker
/// proves the ticker actually ran the chain) — so the real assertion reads
/// the persisted row straight from the DB afterwards and checks BOTH that
/// the beat moved AND that `arm_beat_clock` re-armed a live, future
/// deadline for it. That's what pins "the ticker ran the same chain a human
/// clicking LOCK on time would have", not just "some frame arrived".
#[tokio::test]
async fn test_the_ticker_advances_an_expired_beat() {
    let (app, pool, code, _alice, _bob, _alice_id, _bob_id) = lc_action_rig().await;

    let mut st = lc_state(&pool, &code).await;
    assert_eq!(st.beat, Beat::Lock);
    st.beat_deadline_ms = Some(unix_ms_now() - 2_000);
    let game_id = lc_game_id(&pool, &code).await;
    drinkinggame::db::set_game_state(&pool, game_id, &st.to_json()).await;

    let res = get(&app, &format!("/room/{code}/sse")).await;
    let mut body = res.into_body().into_data_stream();
    read_sse_until(&mut body, r#"data-beat="reveal""#).await;

    let after = lc_state(&pool, &code).await;
    assert_eq!(after.beat, Beat::Reveal);
    assert!(
        after.beat_deadline_ms.is_some_and(|d| d > unix_ms_now()),
        "{:?}",
        after.beat_deadline_ms
    );
}

/// Round 1's Draw is the untimed registration lobby (E1) — `begin` is what
/// arms the clock for the first time. Rigged with no `beat` override: just
/// `start` plus both members registering a vessel through the real routes,
/// the same lobby state a table actually sits in before anyone presses the
/// button. Also covers the brief's other `begin` gate (too few registered):
/// a lone alice presses `begin` and gets 409 before bob has even registered
/// a vessel.
#[tokio::test]
async fn test_begin_starts_the_round_and_arms_diplomacy() {
    let (app, pool) = test_app_with_pool().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;
    post_form(&app, &alice, &format!("/room/{code}/lastcall/start"), "").await;
    post_form(
        &app,
        &alice,
        &format!("/room/{code}/lastcall/vessel"),
        "deck=beer&container=can",
    )
    .await;

    // Only one of two registered: TooFewPlayers, same "not now" 409 as the
    // wrong-beat case below.
    let res = post_form(&app, &alice, &format!("/room/{code}/lastcall/begin"), "").await;
    assert_eq!(res.status(), StatusCode::CONFLICT);
    assert_eq!(lc_state(&pool, &code).await.beat, Beat::Draw); // untouched

    post_form(
        &app,
        &bob,
        &format!("/room/{code}/lastcall/vessel"),
        "deck=cider&container=bottle",
    )
    .await;

    let res = get(&app, &format!("/room/{code}/sse")).await;
    let mut body = res.into_body().into_data_stream();
    read_sse_until(&mut body, "event: lcpublic").await; // drain the snapshot

    // Any member, not just the room owner — the tm_roll_handler precedent.
    let res = post_form(&app, &bob, &format!("/room/{code}/lastcall/begin"), "").await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    read_sse_until(&mut body, r#"data-beat="diplomacy""#).await;
    let after = lc_state(&pool, &code).await;
    assert_eq!(after.beat, Beat::Diplomacy);
    let now = unix_ms_now();
    assert!(
        after
            .beat_deadline_ms
            .is_some_and(|d| d > now && d <= now + 60_000),
        "{:?}",
        after.beat_deadline_ms
    ); // 60s DIPLOMACY_SECS armed

    // Not round-1 Draw any more: a second press is "not now".
    let res = post_form(&app, &alice, &format!("/room/{code}/lastcall/begin"), "").await;
    assert_eq!(res.status(), StatusCode::CONFLICT);
}

/// Decision E3: the all-locked table advances without waiting for the Lock
/// beat's timer. Rigged at `Beat::Lock` (no deadline needed — the advance
/// this test pins is the lock route's own early exit, not the ticker's).
#[tokio::test]
async fn test_locking_the_whole_table_advances_early() {
    let (app, pool, code, alice, bob, _alice_id, _bob_id) = lc_action_rig().await;

    post_form(
        &app,
        &alice,
        &format!("/room/{code}/lastcall/arm"),
        "card_id=beer-01",
    )
    .await;
    post_form(
        &app,
        &alice,
        &format!("/room/{code}/lastcall/target"),
        "card_id=beer-01&target=1", // bob's seat
    )
    .await;

    let res = get(&app, &format!("/room/{code}/sse")).await;
    let mut body = res.into_body().into_data_stream();
    read_sse_until(&mut body, "event: lcpublic").await; // drain the snapshot

    let res = post_form(&app, &alice, &format!("/room/{code}/lastcall/lock"), "").await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    // alice locking alone must not advance — bob hasn't locked yet.
    assert_eq!(lc_state(&pool, &code).await.beat, Beat::Lock);

    // bob locks nothing armed — legal (`lock_in`'s own doc comment).
    let res = post_form(&app, &bob, &format!("/room/{code}/lastcall/lock"), "").await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    // Frame filtered by content, never by position — the second lock's own
    // publish may or may not be the very next SSE chunk.
    read_sse_until(&mut body, r#"data-beat="reveal""#).await;
    let after = lc_state(&pool, &code).await;
    assert_eq!(after.beat, Beat::Reveal);
    assert!(
        after.beat_deadline_ms.is_some_and(|d| d > unix_ms_now()),
        "{:?}",
        after.beat_deadline_ms
    ); // 20s REVEAL_SECS armed
}

/// D16 belt-and-suspenders: `lc_advance_chain` itself clears
/// `beat_deadline_ms` on an outcome, so a game that ends through the normal
/// chain never leaves a live deadline behind for the ticker to trip over.
/// This test forces the OTHER path: a state that (only a hand-corrupted or
/// legacy blob could produce this pairing naturally) carries an EXPIRED
/// deadline *and* an already-decided outcome at the same time, to prove the
/// ticker's own `|| outcome().is_some()` guard — present on both the
/// lock-free advisory pre-check and the locked recheck — is what actually
/// keeps a finished game frozen, not merely the absence of a deadline.
#[tokio::test]
async fn test_ticker_leaves_a_finished_game_frozen() {
    let (app, pool, code, _alice, _bob, _alice_id, _bob_id) = lc_action_rig().await;

    let mut st = lc_state(&pool, &code).await;
    st.beat = Beat::Resolve;
    st.players[1].status = drinkinggame::last_call::Status::Eliminated;
    assert!(st.outcome().is_some());
    let stale_deadline = unix_ms_now() - 2_000;
    st.beat_deadline_ms = Some(stale_deadline);
    let game_id = lc_game_id(&pool, &code).await;
    drinkinggame::db::set_game_state(&pool, game_id, &st.to_json()).await;

    // Subscribing puts the room in active_rooms(), same as every other test
    // here — the ticker only ever looks at hub-active rooms.
    let _res = get(&app, &format!("/room/{code}/sse")).await;
    // Give the 1 Hz ticker several sweeps' worth of real time to (not) act.
    tokio::time::sleep(std::time::Duration::from_millis(2_500)).await;

    let after = lc_state(&pool, &code).await;
    assert_eq!(after.beat, Beat::Resolve);
    assert_eq!(after.round, st.round);
    assert_eq!(after.beat_deadline_ms, Some(stale_deadline)); // untouched
}

// -------------------------------------------------------------
// Plan E (Task 4): the F.1 action bar, the target picker, and lc_loop.js.
// -------------------------------------------------------------

#[tokio::test]
async fn test_lc_loop_js_is_served_and_binds_the_delegated_listeners() {
    let app = test_app().await;
    let res = get(&app, "/assets/lc_loop.js").await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        res.headers()[header::CONTENT_TYPE],
        "application/javascript"
    );
    let js = body_string(res).await;
    assert!(!js.is_empty());
    for needle in [
        "DOMContentLoaded",
        "data-lc-post",
        "data-lc-target",
        "lc:arm",
        "lc:disarm",
        "lcLoopApply",
        "lcLoopPublic",
        "__lcLoopBound",
    ] {
        assert!(js.contains(needle), "missing {needle}");
    }
}

/// The armed column's `LOCKED {n}` bug the STATUS carried notes flagged:
/// `lock_in` empties `p.armed` into `locked_plays`, so reading `p.armed`
/// unconditionally would show a just-locked viewer's own header as
/// `LOCKED 0`. This also doubles as the Step 5 acceptance test — alice's
/// hand carries the action-bar template and the locked hint; bob's carries
/// LOCK IN and none of alice's locked copy.
#[tokio::test]
async fn test_hand_fetch_carries_the_action_template_per_viewer() {
    let (app, _pool, code, alice, bob, _alice_id, _bob_id) = lc_action_rig().await;

    post_form(
        &app,
        &alice,
        &format!("/room/{code}/lastcall/arm"),
        "card_id=beer-01",
    )
    .await;
    post_form(
        &app,
        &alice,
        &format!("/room/{code}/lastcall/target"),
        "card_id=beer-01&target=1", // bob's seat; beer-01 targets "one"
    )
    .await;
    let res = post_form(&app, &alice, &format!("/room/{code}/lastcall/lock"), "").await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    let alice_hand = body_string(get_hand(&app, &alice, &code).await).await;
    assert!(
        alice_hand.contains("<template data-lc-actions>"),
        "{alice_hand}"
    );
    assert!(
        alice_hand.contains("LOCKED — WAITING FOR THE TABLE"),
        "{alice_hand}"
    );
    // The Plan D seam this task closes: a locked viewer's own armed column
    // still shows their staged card, not LOCKED 0.
    assert!(alice_hand.contains("LOCKED 1"), "{alice_hand}");
    assert!(alice_hand.contains("beer-01"), "{alice_hand}");

    let bob_hand = body_string(get_hand(&app, &bob, &code).await).await;
    assert!(bob_hand.contains("LOCK IN"), "{bob_hand}");
    assert!(!bob_hand.contains("LOCKED — WAITING"), "{bob_hand}");
}

/// Decision E8: the target picker lists only Alive seats and posts the
/// choice back through `/lastcall/target`. cara is seated but eliminated —
/// she must never appear as a pickable target, even though her seat exists.
#[tokio::test]
async fn test_target_picker_lists_only_alive_seats_and_posts_back() {
    let (app, pool) = test_app_with_pool().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let _cara = login(&app, "cara", "1234").await; // never joins the room
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;
    post_form(&app, &alice, &format!("/room/{code}/lastcall/start"), "").await;

    let alice_id = drinkinggame::db::get_player_by_name(&pool, "alice")
        .await
        .unwrap()
        .id;
    let bob_id = drinkinggame::db::get_player_by_name(&pool, "bob")
        .await
        .unwrap()
        .id;
    let cara_id = drinkinggame::db::get_player_by_name(&pool, "cara")
        .await
        .unwrap()
        .id;

    let mut st = LastCallState::new(
        vec![
            (alice_id, "alice".into()),
            (bob_id, "bob".into()),
            (cara_id, "cara".into()),
        ],
        7,
    );
    st.set_vessel(alice_id, Deck::Beer, "can").unwrap();
    st.set_vessel(bob_id, Deck::Cider, "bottle").unwrap();
    st.players[2].status = drinkinggame::last_call::Status::Eliminated;
    st.beat = Beat::Lock;
    let game_id = lc_game_id(&pool, &code).await;
    drinkinggame::db::set_game_state(&pool, game_id, &st.to_json()).await;

    // beer-01 targets "one".
    post_form(
        &app,
        &alice,
        &format!("/room/{code}/lastcall/arm"),
        "card_id=beer-01",
    )
    .await;

    // Scoped to the .lc-targets section itself, not the whole fragment —
    // "cara" legitimately appears elsewhere in the pane (the handicap rows
    // list every seated player regardless of status), so a whole-fragment
    // `!contains("cara")` would pass or fail for the wrong reason.
    fn targets_section(hand: &str) -> &str {
        let start = hand
            .find(r#"<section class="lc-targets">"#)
            .expect("no .lc-targets section");
        let rest = &hand[start..];
        let end = rest
            .find("</section>")
            .expect("unterminated .lc-targets section");
        &rest[..end]
    }

    let hand = body_string(get_hand(&app, &alice, &code).await).await;
    let section = targets_section(&hand);
    assert!(section.contains("data-lc-target"), "{section}");
    assert!(section.contains("bob"), "{section}");
    assert!(!section.contains("cara"), "{section}");

    let res = post_form(
        &app,
        &alice,
        &format!("/room/{code}/lastcall/target"),
        "card_id=beer-01&target=1", // bob's seat
    )
    .await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    let hand = body_string(get_hand(&app, &alice, &code).await).await;
    let section = targets_section(&hand);
    assert!(section.contains(r#"value="1" selected"#), "{section}");
}

/// E9: the reveal charge is priced through the VIEWER'S OWN handicap, not a
/// shared constant — same property Plan C's per-viewer cost rail pins, one
/// level up (the action bar's DRINK n, not the rail's bars). alice and bob
/// each arm a cost-2 card at the default 100% handicap; bob's is then rigged
/// to 150% before he locks (handicap is Draw-gated, so this edits the
/// persisted state directly rather than going through the route — the same
/// pattern `test_ticker_leaves_a_finished_game_frozen` uses).
#[tokio::test]
async fn test_reveal_charge_is_priced_per_viewer() {
    let (app, pool, code, alice, bob, _alice_id, bob_id) = lc_action_rig().await;

    // beer-02, cost 2, targets "one".
    post_form(
        &app,
        &alice,
        &format!("/room/{code}/lastcall/arm"),
        "card_id=beer-02",
    )
    .await;
    post_form(
        &app,
        &alice,
        &format!("/room/{code}/lastcall/target"),
        "card_id=beer-02&target=1", // bob's seat
    )
    .await;
    let res = post_form(&app, &alice, &format!("/room/{code}/lastcall/lock"), "").await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    let mut st = lc_state(&pool, &code).await;
    let bob_seat = st.seat_of(bob_id).unwrap();
    st.players[bob_seat].handicap_pct = 150;
    let game_id = lc_game_id(&pool, &code).await;
    drinkinggame::db::set_game_state(&pool, game_id, &st.to_json()).await;

    // cider-03, also cost 2, targets "one".
    post_form(
        &app,
        &bob,
        &format!("/room/{code}/lastcall/arm"),
        "card_id=cider-03",
    )
    .await;
    post_form(
        &app,
        &bob,
        &format!("/room/{code}/lastcall/target"),
        "card_id=cider-03&target=0", // alice's seat
    )
    .await;
    let res = post_form(&app, &bob, &format!("/room/{code}/lastcall/lock"), "").await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT); // all locked -> auto-advance to Reveal

    let after = lc_state(&pool, &code).await;
    assert_eq!(after.beat, Beat::Reveal);

    // alice: cost 2 at her own 100% handicap -> DRINK 2.
    let alice_hand = body_string(get_hand(&app, &alice, &code).await).await;
    assert!(alice_hand.contains("DRINK 2"), "{alice_hand}");

    // bob: the SAME cost-2 shape, priced through HIS OWN 150% handicap ->
    // DRINK 3 — two prices for the same nominal cost, proof the charge is
    // per-viewer, not shared.
    let bob_hand = body_string(get_hand(&app, &bob, &code).await).await;
    assert!(bob_hand.contains("DRINK 3"), "{bob_hand}");
}

// -------------------------------------------------------------
// Last Call (Plan E Task 5): the reveal on the felt — end to end through the
// real routes, not just the renderer's own unit tests.
// -------------------------------------------------------------

/// Mirrors `test_locking_the_whole_table_advances_early`'s rig (alice arms
/// and targets beer-01, bob locks nothing), but checks what the SAME
/// all-locked advance publishes on the wire: the Lock -> Reveal frame must
/// carry the felt centre with the played card's title, public exactly now
/// (§3.4.1) — and the existing end-of-game handoff
/// (`test_ending_last_call_keeps_the_room_open`'s property) must still hold
/// with the loop live, not just at the untouched Draw lobby that test rig
/// used.
#[tokio::test]
async fn test_the_reveal_frame_carries_the_plays_and_the_end_still_hands_off() {
    let (app, _pool, code, alice, bob, _alice_id, _bob_id) = lc_action_rig().await;

    post_form(
        &app,
        &alice,
        &format!("/room/{code}/lastcall/arm"),
        "card_id=beer-01",
    )
    .await;
    post_form(
        &app,
        &alice,
        &format!("/room/{code}/lastcall/target"),
        "card_id=beer-01&target=1", // bob's seat
    )
    .await;
    let res = post_form(&app, &alice, &format!("/room/{code}/lastcall/lock"), "").await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    let res = get(&app, &format!("/room/{code}/sse")).await;
    let mut body = res.into_body().into_data_stream();
    read_sse_until(&mut body, "event: lcpublic").await; // drain the snapshot

    // bob locks nothing armed — legal — which is also the all-locked seat
    // and triggers the E3 early advance (Lock -> Reveal) inside the handler.
    let res = post_form(&app, &bob, &format!("/room/{code}/lastcall/lock"), "").await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    // Frame filtered by content, never by position (Task 4's own fix
    // applies here too — the second lock's publish may not be the very
    // next chunk). Waits on "Nudge" (beer-01's title), not the earlier
    // "lc-centre-plays" wrapper class: a big fragment can split across SSE
    // chunks (`read_sse_until`'s own doc comment), and the title sits later
    // in the same string, so waiting on it guarantees both substrings have
    // actually arrived by the time this returns.
    let seen = read_sse_until(&mut body, "Nudge").await;
    let frame = &seen[seen.find("event: lcpublic").unwrap()..];
    assert!(frame.contains("lc-centre-plays"), "{frame}");
    assert!(frame.contains("Nudge"), "{frame}"); // beer-01's title — identity is public exactly now

    let end_res = post_form(&app, &alice, &format!("/room/{code}/lastcall/end"), "").await;
    assert_eq!(end_res.status(), StatusCode::NO_CONTENT);

    let seen = read_sse_until(&mut body, "event: game").await;
    let game_frame = &seen[seen.find("event: game").unwrap()..];
    assert!(
        matches_the_game_idle_selector(game_frame),
        "the existing .game-idle handoff must still fire with the loop \
         live: {game_frame}"
    );
}

// -------------------------------------------------------------
// Plan G (Task 4): the pact routes — offer/accept/decline — and the
// mandatory secrecy proof: a pact between two seats never reaches the wire
// (`lcpublic`), a third seat's own private fragment, or the spectator
// screen (which subscribes only to `lcpublic` — see
// `test_lcpublic_never_carries_hand_cards` for that channel's own proof).
// -------------------------------------------------------------

/// THE MANDATORY TEST (brief; spec §3.4.1's shape): a pact between A and B
/// is absent from `public_view()`'s output on the wire, and C's private
/// fragment is unchanged by its existence.
#[tokio::test]
async fn test_a_pact_between_a_and_b_is_invisible_to_c_and_the_wire() {
    let (app, pool) = test_app_with_pool().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let cara = login(&app, "cara", "1111").await;
    let dave = login(&app, "dave", "2222").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;
    room_page_html(&app, &cara, &code).await;
    room_page_html(&app, &dave, &code).await;
    post_form(&app, &alice, &format!("/room/{code}/lastcall/start"), "").await;

    let alice_id = drinkinggame::db::get_player_by_name(&pool, "alice")
        .await
        .unwrap()
        .id;
    let bob_id = drinkinggame::db::get_player_by_name(&pool, "bob")
        .await
        .unwrap()
        .id;
    let cara_id = drinkinggame::db::get_player_by_name(&pool, "cara")
        .await
        .unwrap()
        .id;
    let dave_id = drinkinggame::db::get_player_by_name(&pool, "dave")
        .await
        .unwrap()
        .id;
    let game_id = lc_game_id(&pool, &code).await;

    // alice(0)/bob(1)/cara(2)/dave(3), vessels registered, at Diplomacy —
    // the same real-id rig `test_the_hand_fragment_shows_only_the_viewers_own_pact`
    // (Task 3) uses, so `offer_pact`/`accept_pact` behind the real routes
    // resolve seats correctly.
    let mut st = LastCallState::new(
        vec![
            (alice_id, "alice".into()),
            (bob_id, "bob".into()),
            (cara_id, "cara".into()),
            (dave_id, "dave".into()),
        ],
        1,
    );
    st.set_vessel(alice_id, Deck::Beer, "can").unwrap();
    st.set_vessel(bob_id, Deck::Cider, "bottle").unwrap();
    st.set_vessel(cara_id, Deck::Soft, "glass").unwrap();
    st.set_vessel(dave_id, Deck::Liquor, "shot").unwrap();
    st.beat = Beat::Diplomacy;
    drinkinggame::db::set_game_state(&pool, game_id, &st.to_json()).await;

    let cara_before = body_string(get_hand(&app, &cara, &code).await).await;

    let res = get(&app, &format!("/room/{code}/sse")).await;
    let mut body = res.into_body().into_data_stream();
    read_sse_until(&mut body, "event: lcpublic").await; // drain the snapshot

    let res = post_form(
        &app,
        &alice,
        &format!("/room/{code}/lastcall/pact/offer"),
        "target=1", // bob's seat
    )
    .await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    let seen = read_sse_until(&mut body, "event: lctick").await;
    assert!(
        !seen.contains("event: lcpublic")
            && !seen.contains("event: game")
            && !seen.contains("event: room"),
        "{seen}"
    );

    let res = post_form(
        &app,
        &bob,
        &format!("/room/{code}/lastcall/pact/accept"),
        "from=0", // alice's seat
    )
    .await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    let seen = read_sse_until(&mut body, "event: lctick").await;
    assert!(
        !seen.contains("event: lcpublic")
            && !seen.contains("event: game")
            && !seen.contains("event: room"),
        "{seen}"
    );

    let alice_hand = body_string(get_hand(&app, &alice, &code).await).await;
    assert!(
        alice_hand.contains("PACT WITH BOB — SINCE ROUND 1"),
        "{alice_hand}"
    );
    let bob_hand = body_string(get_hand(&app, &bob, &code).await).await;
    assert!(bob_hand.contains("PACT WITH ALICE"), "{bob_hand}");

    let cara_after = body_string(get_hand(&app, &cara, &code).await).await;
    assert_eq!(
        without_seq(&cara_before),
        without_seq(&cara_after),
        "cara's world must be byte-identical whether or not alice and bob pacted"
    );

    // The third named surface: the spectator screen, which needs no session
    // at all (`test_the_last_call_screen_needs_no_session`) — the pact must
    // be invisible there too, not just to a fellow player.
    let screen = body_string(get(&app, &format!("/room/{code}/screen")).await).await;
    assert!(!screen.contains("PACT WITH"), "{screen}");
    assert!(!screen.contains("lc-pact-standing"), "{screen}");

    // Force the beat to Draw via a direct state write (the pact itself is
    // beat-independent — only `formed_round` is stamped, not the beat it
    // formed in) so `/lastcall/handicap` — a full-publish route with
    // nothing to do with pacts — can run without `WrongBeat`, and use it to
    // prove the broadcast surface never carries pact state even while a
    // pact still exists.
    let mut st2 = lc_state(&pool, &code).await;
    assert!(!st2.pacts.is_empty()); // premise: the pact still exists
    st2.beat = Beat::Draw;
    drinkinggame::db::set_game_state(&pool, game_id, &st2.to_json()).await;

    let res = post_form(
        &app,
        &alice,
        &format!("/room/{code}/lastcall/handicap"),
        &format!("target={alice_id}&handicap_pct=100"),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    let seen = read_sse_until(&mut body, "event: lcpublic").await;
    let frame = &seen[seen.find("event: lcpublic").unwrap()..];
    assert!(!frame.contains("PACT WITH"), "{frame}");
    assert!(!frame.contains("lc-pact-standing"), "{frame}");
    assert!(!frame.contains("lc-pacts"), "{frame}");
}

/// The guard chain (member -> active game -> kind -> beat, all from
/// `load_lc`/the engine) and the error-mapping table (`PactBlocked` ->
/// "No pact to be had.", `NoOffer` -> "That offer is gone.") for all three
/// pact routes, plus the G11 no-pact-detector property: an offer TO a
/// pacted seat is a silent 204 no-op, never an error that would leak "that
/// seat is unavailable" to the offeror.
#[tokio::test]
async fn test_pact_routes_are_guarded_and_answer_in_words() {
    let (app, pool) = test_app_with_pool().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let cara = login(&app, "cara", "1111").await;
    let dave = login(&app, "dave", "2222").await;
    let carol = login(&app, "carol", "3333").await; // never opens the room: not a member
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;
    room_page_html(&app, &cara, &code).await;
    room_page_html(&app, &dave, &code).await;
    post_form(&app, &alice, &format!("/room/{code}/lastcall/start"), "").await;

    // carol never opened the room: not a member -> 403 (`load_lc`).
    let res = post_form(
        &app,
        &carol,
        &format!("/room/{code}/lastcall/pact/offer"),
        "target=1",
    )
    .await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    post_form(&app, &alice, &format!("/room/{code}/lastcall/end"), "").await;

    // A 3 Man game active in the same room: WrongGameKind -> 409.
    post_form(&app, &alice, &format!("/room/{code}/tm/start"), "").await;
    let res = post_form(
        &app,
        &alice,
        &format!("/room/{code}/lastcall/pact/offer"),
        "target=1",
    )
    .await;
    assert_eq!(res.status(), StatusCode::CONFLICT);
    post_form(&app, &alice, &format!("/room/{code}/tm/end"), "").await;

    // Restart Last Call with all four seated, force the state to real ids
    // at Beat::Lock: WrongBeat -> 409 (OutOfTurn — `map_lc`'s WrongBeat arm).
    post_form(&app, &alice, &format!("/room/{code}/lastcall/start"), "").await;
    let alice_id = drinkinggame::db::get_player_by_name(&pool, "alice")
        .await
        .unwrap()
        .id;
    let bob_id = drinkinggame::db::get_player_by_name(&pool, "bob")
        .await
        .unwrap()
        .id;
    let cara_id = drinkinggame::db::get_player_by_name(&pool, "cara")
        .await
        .unwrap()
        .id;
    let dave_id = drinkinggame::db::get_player_by_name(&pool, "dave")
        .await
        .unwrap()
        .id;
    let game_id = lc_game_id(&pool, &code).await;

    let mut st = LastCallState::new(
        vec![
            (alice_id, "alice".into()),
            (bob_id, "bob".into()),
            (cara_id, "cara".into()),
            (dave_id, "dave".into()),
        ],
        1,
    );
    st.set_vessel(alice_id, Deck::Beer, "can").unwrap();
    st.set_vessel(bob_id, Deck::Cider, "bottle").unwrap();
    st.set_vessel(cara_id, Deck::Soft, "glass").unwrap();
    st.set_vessel(dave_id, Deck::Liquor, "shot").unwrap();
    st.beat = Beat::Lock;
    drinkinggame::db::set_game_state(&pool, game_id, &st.to_json()).await;

    let res = post_form(
        &app,
        &alice,
        &format!("/room/{code}/lastcall/pact/offer"),
        "target=1",
    )
    .await;
    assert_eq!(res.status(), StatusCode::CONFLICT); // WrongBeat -> OutOfTurn

    // At Diplomacy: accept a nonexistent offer -> 422, body exactly "That
    // offer is gone.".
    st.beat = Beat::Diplomacy;
    drinkinggame::db::set_game_state(&pool, game_id, &st.to_json()).await;

    let res = post_form(
        &app,
        &alice,
        &format!("/room/{code}/lastcall/pact/accept"),
        "from=3", // dave never offered
    )
    .await;
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body_string(res).await, "That offer is gone.");

    // alice+bob pact, formed through the real routes.
    let res = post_form(
        &app,
        &alice,
        &format!("/room/{code}/lastcall/pact/offer"),
        "target=1",
    )
    .await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    let res = post_form(
        &app,
        &bob,
        &format!("/room/{code}/lastcall/pact/accept"),
        "from=0",
    )
    .await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    // G11: cara offering to the now-pacted alice is a silent no-op, not an
    // error — the market list must never double as a pact detector.
    let res = post_form(
        &app,
        &cara,
        &format!("/room/{code}/lastcall/pact/offer"),
        "target=0",
    )
    .await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    // alice (the pacted OFFEROR) trying to open a second pact -> 422, body
    // exactly "No pact to be had.".
    let res = post_form(
        &app,
        &alice,
        &format!("/room/{code}/lastcall/pact/offer"),
        "target=2", // cara's seat
    )
    .await;
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body_string(res).await, "No pact to be had.");
}

/// A decline clears the offer for both phones: the target's row disappears
/// and the offeror's own WAITING line reverts to a propose button — no
/// error, tick-only broadcast (the same E5 policy assertion arm/disarm's
/// test makes for its own routes).
#[tokio::test]
async fn test_decline_clears_the_offer_for_both_phones() {
    let (app, pool) = test_app_with_pool().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let cara = login(&app, "cara", "1111").await;
    let dave = login(&app, "dave", "2222").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;
    room_page_html(&app, &cara, &code).await;
    room_page_html(&app, &dave, &code).await;
    post_form(&app, &alice, &format!("/room/{code}/lastcall/start"), "").await;

    let alice_id = drinkinggame::db::get_player_by_name(&pool, "alice")
        .await
        .unwrap()
        .id;
    let bob_id = drinkinggame::db::get_player_by_name(&pool, "bob")
        .await
        .unwrap()
        .id;
    let cara_id = drinkinggame::db::get_player_by_name(&pool, "cara")
        .await
        .unwrap()
        .id;
    let dave_id = drinkinggame::db::get_player_by_name(&pool, "dave")
        .await
        .unwrap()
        .id;
    let game_id = lc_game_id(&pool, &code).await;

    let mut st = LastCallState::new(
        vec![
            (alice_id, "alice".into()),
            (bob_id, "bob".into()),
            (cara_id, "cara".into()),
            (dave_id, "dave".into()),
        ],
        1,
    );
    st.set_vessel(alice_id, Deck::Beer, "can").unwrap();
    st.set_vessel(bob_id, Deck::Cider, "bottle").unwrap();
    st.set_vessel(cara_id, Deck::Soft, "glass").unwrap();
    st.set_vessel(dave_id, Deck::Liquor, "shot").unwrap();
    st.beat = Beat::Diplomacy;
    drinkinggame::db::set_game_state(&pool, game_id, &st.to_json()).await;

    let res = get(&app, &format!("/room/{code}/sse")).await;
    let mut body = res.into_body().into_data_stream();
    read_sse_until(&mut body, "event: lcpublic").await; // drain the snapshot

    let res = post_form(
        &app,
        &alice,
        &format!("/room/{code}/lastcall/pact/offer"),
        "target=1", // bob's seat
    )
    .await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    read_sse_until(&mut body, "event: lctick").await;

    let bob_hand = body_string(get_hand(&app, &bob, &code).await).await;
    assert!(bob_hand.contains("ALICE OFFERS A PACT"), "{bob_hand}");
    assert!(bob_hand.contains(r#"data-lc-body="from=0""#), "{bob_hand}");

    let res = post_form(
        &app,
        &bob,
        &format!("/room/{code}/lastcall/pact/decline"),
        "from=0",
    )
    .await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    let seen = read_sse_until(&mut body, "event: lctick").await;
    assert!(
        !seen.contains("event: lcpublic")
            && !seen.contains("event: game")
            && !seen.contains("event: room"),
        "{seen}"
    );

    let bob_hand = body_string(get_hand(&app, &bob, &code).await).await;
    assert!(!bob_hand.contains("ALICE OFFERS A PACT"), "{bob_hand}");
    assert!(!bob_hand.contains("ACCEPT"), "{bob_hand}");

    let alice_hand = body_string(get_hand(&app, &alice, &code).await).await;
    assert!(!alice_hand.contains("WAITING"), "{alice_hand}");
    assert!(alice_hand.contains("PROPOSE TO BOB"), "{alice_hand}");
}

/// G5/G10 together, at the wire: a betrayal (`pact_breaks`) is the only
/// pact-shaped thing `lcpublic` ever carries — an intact pact in the very
/// same state stays completely absent, even from a route (`/lastcall/
/// handicap`) that has nothing to do with pacts and publishes unconditionally.
#[tokio::test]
async fn test_the_break_is_the_only_public_pact_trace() {
    let (app, pool) = test_app_with_pool().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let cara = login(&app, "cara", "1111").await;
    let dave = login(&app, "dave", "2222").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;
    room_page_html(&app, &cara, &code).await;
    room_page_html(&app, &dave, &code).await;
    post_form(&app, &alice, &format!("/room/{code}/lastcall/start"), "").await;

    let alice_id = drinkinggame::db::get_player_by_name(&pool, "alice")
        .await
        .unwrap()
        .id;
    let bob_id = drinkinggame::db::get_player_by_name(&pool, "bob")
        .await
        .unwrap()
        .id;
    let cara_id = drinkinggame::db::get_player_by_name(&pool, "cara")
        .await
        .unwrap()
        .id;
    let dave_id = drinkinggame::db::get_player_by_name(&pool, "dave")
        .await
        .unwrap()
        .id;
    let game_id = lc_game_id(&pool, &code).await;

    // Round 1 (default beat: Draw, so `/lastcall/handicap` is legal): one
    // betrayal on record for round 1 (seats 0 -> 1) AND an intact pact
    // between seats 2 and 3, in the same state.
    let mut st = LastCallState::new(
        vec![
            (alice_id, "alice".into()),
            (bob_id, "bob".into()),
            (cara_id, "cara".into()),
            (dave_id, "dave".into()),
        ],
        1,
    );
    st.pact_breaks.push(drinkinggame::last_call::PactBreak {
        betrayer: 0,
        betrayed: 1,
        round: 1,
    });
    st.pacts.push(drinkinggame::last_call::Pact {
        a: 2,
        b: 3,
        formed_round: 1,
    });
    drinkinggame::db::set_game_state(&pool, game_id, &st.to_json()).await;

    let res = get(&app, &format!("/room/{code}/sse")).await;
    let mut body = res.into_body().into_data_stream();
    read_sse_until(&mut body, "event: lcpublic").await; // drain the snapshot

    let res = post_form(
        &app,
        &alice,
        &format!("/room/{code}/lastcall/handicap"),
        &format!("target={alice_id}&handicap_pct=100"),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    let seen = read_sse_until(&mut body, "event: lcpublic").await;
    let frame = &seen[seen.find("event: lcpublic").unwrap()..];
    assert!(frame.contains("lc-pact-break"), "{frame}");
    assert!(frame.contains("BROKE THEIR PACT WITH"), "{frame}");
    assert!(!frame.contains("lc-pact-standing"), "{frame}");
    assert!(!frame.contains("SINCE ROUND"), "{frame}");
}

// -------------------------------------------------------------
// Plan H, Task 4: the banner strip — one occupant at a time, on every
// surface the `lcpublic` frame reaches (H6/H11).
// -------------------------------------------------------------

/// The round's event rides the wire from the Deal reveal onward, but the
/// tab dealt at that very same edge (H7's replacement deal) never does —
/// H2's event is public, H8's tabs stay private regardless. `begin`'s own
/// advance chain (Draw -> Deal -> Diplomacy, decision E4) runs the whole
/// reveal server-side before the one publish at the end (`lc_advance_chain`
/// only calls `persist_and_broadcast_lc` once), so there is exactly one
/// `lcpublic` frame to inspect here, not several.
#[tokio::test]
async fn test_the_deal_reveals_exactly_one_event_on_the_wire() {
    let (app, _pool) = test_app_with_pool().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;
    post_form(&app, &alice, &format!("/room/{code}/lastcall/start"), "").await;
    post_form(
        &app,
        &alice,
        &format!("/room/{code}/lastcall/vessel"),
        "deck=beer&container=can",
    )
    .await;
    post_form(
        &app,
        &bob,
        &format!("/room/{code}/lastcall/vessel"),
        "deck=cider&container=bottle",
    )
    .await;

    let res = get(&app, &format!("/room/{code}/sse")).await;
    let mut body = res.into_body().into_data_stream();
    read_sse_until(&mut body, "event: lcpublic").await; // drain the snapshot

    let res = post_form(&app, &alice, &format!("/room/{code}/lastcall/begin"), "").await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    // The lcpublic frame's banner template carries the strip (one root,
    // Plan A2's `<template data-lc-banner>` — both the phone shell and the
    // screen swap their own `#lc-banner` from this same rendered string, so
    // it only needs to appear once on the wire).
    let seen = read_sse_until(&mut body, "data-event=").await;
    assert_eq!(seen.matches("data-event=").count(), 1, "{seen}");
    assert!(!seen.contains("SETTLED A TAB"), "{seen}");
    for banned in ["lie-low", "LIE LOW", "high-roller", "HIGH ROLLER"] {
        assert!(!seen.contains(banned), "leaked {banned}: {seen}");
    }

    // The screen's own server-rendered initial banner carries the same
    // single occurrence and the same absences.
    let screen = body_string(get(&app, &format!("/room/{code}/screen")).await).await;
    assert_eq!(screen.matches("data-event=").count(), 1, "{screen}");
    assert!(!screen.contains("SETTLED A TAB"), "{screen}");
    for banned in ["lie-low", "LIE LOW", "high-roller", "HIGH ROLLER"] {
        assert!(!screen.contains(banned), "leaked {banned}: {screen}");
    }
}

/// H11: the announcement is the settling player's name, never the tab id or
/// title — H8's privacy line holds even after the tab is gone and done.
/// Rigged directly at round 2 Beat::Draw so `public_view()`'s `settled`
/// filter (`t.round + 1 == self.round`) picks up a round-1 settle by
/// alice's real seat (0, per `LastCallState::new`'s seating order).
#[tokio::test]
async fn test_a_settlement_announces_the_name_not_the_tab() {
    let (app, pool) = test_app_with_pool().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;
    post_form(&app, &alice, &format!("/room/{code}/lastcall/start"), "").await;

    let alice_id = drinkinggame::db::get_player_by_name(&pool, "alice")
        .await
        .unwrap()
        .id;
    let bob_id = drinkinggame::db::get_player_by_name(&pool, "bob")
        .await
        .unwrap()
        .id;

    let mut st = LastCallState::new(vec![(alice_id, "alice".into()), (bob_id, "bob".into())], 7);
    st.round = 2;
    st.beat = Beat::Draw;
    // Plan H Task 5: seed 7 deals alice (seat 0) the "lie-low" tab by
    // construction (`tab_for(7, 0, 0)` == `TABS[0]`) — clear her *current*
    // tab so the private hand card (now rendering it, correctly, since it's
    // her own) can't collide with this test's actual subject: the ledger
    // entry's id must never leak into the public announcement below.
    st.players[0].tabs = vec![];
    st.tab_ledger.push(drinkinggame::last_call::TabSettle {
        seat: 0, // alice's real seat
        tab: "lie-low".to_string(),
        round: 1,
    });
    let game_id = lc_game_id(&pool, &code).await;
    drinkinggame::db::set_game_state(&pool, game_id, &st.to_json()).await;

    let shell = body_string(get_shell(&app, &alice, &code).await).await;
    assert!(shell.contains("ALICE SETTLED A TAB"), "{shell}");
    assert!(!shell.contains("lie-low"), "{shell}");
    assert!(!shell.contains("LIE LOW"), "{shell}");

    let screen = body_string(get(&app, &format!("/room/{code}/screen")).await).await;
    assert!(screen.contains("ALICE SETTLED A TAB"), "{screen}");
    assert!(!screen.contains("lie-low"), "{screen}");
    assert!(!screen.contains("LIE LOW"), "{screen}");
}

/// Review fix round 1, item 3 (⚠️ 1 / erratum a2e66ab): a settle in the very
/// round the game ends on must not go unannounced. `public_view().settled`'s
/// `ended && t.round == self.round` arm (a2e66ab) exists exactly for this —
/// a resolve() that ends the game returns before the round rollover a
/// non-terminal settle would otherwise ride to the "round after" — but
/// `settled` has exactly one consumer, `lc_banner`, and Task 4's rule 1
/// (H6/E13) used to suppress the whole strip whenever `outcome.is_some()`,
/// making the erratum's arm permanently unreachable on every surface. The
/// event chip still yields to the victory presentation (never
/// "HAPPY HOUR"/`data-event` here) — only the settle NAME survives onto the
/// frozen tableau, on both the phone shell and the screen.
#[tokio::test]
async fn test_a_final_round_settle_reaches_the_frozen_game_over_banner() {
    let (app, pool, code, alice, bob, _alice_id, _bob_id) = lc_action_rig().await;

    let mut st = lc_state(&pool, &code).await;
    st.beat = Beat::Resolve;
    st.beat_deadline_ms = None;
    st.players[1].status = drinkinggame::last_call::Status::Eliminated; // bob out -> alice (seat 0) wins
    st.event = Some("happy-hour".to_string()); // must not reach either surface
                                               // Plan H Task 5: `lc_action_rig`'s seed 7 deals alice (seat 0) the
                                               // "lie-low" tab by construction — clear her *current* tab so the
                                               // private hand card (now rendering it, correctly, on her own shell)
                                               // can't collide with this test's actual subject: the ledger entry's id
                                               // must never leak into the public game-over announcement below.
    st.players[0].tabs = vec![];
    st.tab_ledger.push(drinkinggame::last_call::TabSettle {
        seat: 0, // alice's real seat
        tab: "lie-low".to_string(),
        round: st.round, // settled in the game-ending round itself
    });
    assert!(st.outcome().is_some());
    // Premise for the ghost-viewer assertion below: this rig sets
    // `Status::Eliminated` by direct field write, bypassing the engine's own
    // elimination path (`tabs.clear()`, `last_call.rs`) — so bob still HAS a
    // tabs entry. That makes the absence of his tab card unexplainable by
    // `lc_tab_panel(None)`'s placeholder branch; only `hand_pane_html`'s
    // `Status::Alive` gate (`lc_routes.rs`) can suppress it (review fix I1 —
    // `test_a_settled_tab_shows_the_placeholder_card` used an Alive viewer
    // with empty tabs, which exercises the builder's `None` branch, not this
    // gate).
    assert!(!st.players[1].tabs.is_empty());
    let game_id = lc_game_id(&pool, &code).await;
    drinkinggame::db::set_game_state(&pool, game_id, &st.to_json()).await;

    // Review fix I1: bob (seat 1) is Eliminated — no panel at all, not even
    // the settled placeholder, even though he still holds a live tab id.
    let bob_hand = body_string(get_hand(&app, &bob, &code).await).await;
    assert!(!bob_hand.contains("lc-tabcard"), "{bob_hand}");

    let shell = body_string(get_shell(&app, &alice, &code).await).await;
    assert!(shell.contains("GAME OVER"), "{shell}");
    assert!(shell.contains("ALICE SETTLED A TAB"), "{shell}");
    assert!(!shell.contains("HAPPY HOUR"), "{shell}");
    assert!(!shell.contains(r#"data-event"#), "{shell}");
    assert!(!shell.contains("lie-low"), "{shell}");
    assert!(!shell.contains("LIE LOW"), "{shell}");

    let screen = body_string(get(&app, &format!("/room/{code}/screen")).await).await;
    assert!(screen.contains("GAME OVER"), "{screen}");
    assert!(screen.contains("ALICE SETTLED A TAB"), "{screen}");
    assert!(!screen.contains("HAPPY HOUR"), "{screen}");
    assert!(!screen.contains(r#"data-event"#), "{screen}");
    assert!(!screen.contains("lie-low"), "{screen}");
    assert!(!screen.contains("LIE LOW"), "{screen}");
}

// -------------------------------------------------------------
// Plan H, Task 5: the private tab card — the hand fragment is the ONLY
// surface tab identity may render on, and only the viewer's own (H13).
// -------------------------------------------------------------

/// The plan's secrecy gate for tabs, mirroring the pact secrecy tests'
/// shape (`test_a_pact_between_a_and_b_is_invisible_to_c_and_the_wire`):
/// alice's own hand fragment carries alice's tab and none of bob's, bob's
/// carries the reverse, the public wire never carries either tab (same
/// transport the pact proof used — a full-publish route with nothing to do
/// with tabs), and the TABLE fragment stays tab-free.
#[tokio::test]
async fn test_a_tab_is_visible_to_its_holder_alone() {
    let (app, pool, code, alice, bob, alice_id, _bob_id) = lc_action_rig().await;

    // `set_handicap` (the full-publish route used below to prove the wire)
    // requires Beat::Draw — `lc_action_rig` leaves the rig at Lock, so pin
    // tabs deterministically at Draw instead of the room's rolled seed.
    let mut st = lc_state(&pool, &code).await;
    st.beat = Beat::Draw;
    st.players[0].tabs = vec!["lie-low".to_string()]; // alice, seat 0
    st.players[1].tabs = vec!["showboat".to_string()]; // bob, seat 1
    let game_id = lc_game_id(&pool, &code).await;
    drinkinggame::db::set_game_state(&pool, game_id, &st.to_json()).await;

    let alice_hand = body_string(get_hand(&app, &alice, &code).await).await;
    assert!(alice_hand.contains(r#"data-tab="lie-low""#), "{alice_hand}");
    assert!(alice_hand.contains("LIE LOW"), "{alice_hand}");
    assert!(!alice_hand.contains("showboat"), "{alice_hand}");
    assert!(!alice_hand.contains("SHOWBOAT"), "{alice_hand}");

    let bob_hand = body_string(get_hand(&app, &bob, &code).await).await;
    assert!(bob_hand.contains("SHOWBOAT"), "{bob_hand}");
    assert!(!bob_hand.contains("lie-low"), "{bob_hand}");
    assert!(!bob_hand.contains("LIE LOW"), "{bob_hand}");

    let res = get(&app, &format!("/room/{code}/sse")).await;
    let mut body = res.into_body().into_data_stream();
    read_sse_until(&mut body, "event: lcpublic").await; // drain the snapshot

    let res = post_form(
        &app,
        &alice,
        &format!("/room/{code}/lastcall/handicap"),
        &format!("target={alice_id}&handicap_pct=100"),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let seen = read_sse_until(&mut body, "event: lcpublic").await;
    let frame = &seen[seen.find("event: lcpublic").unwrap()..];
    for banned in ["lie-low", "LIE LOW", "showboat", "SHOWBOAT", "lc-tabcard"] {
        assert!(!frame.contains(banned), "leaked {banned}: {frame}");
    }

    let table = body_string(get_table(&app, &alice, &code).await).await;
    assert!(!table.contains("lc-tabcard"), "{table}");
    assert!(!table.contains("lie-low"), "{table}");
    assert!(!table.contains("LIE LOW"), "{table}");
}

/// A settled (or pre-Deal) tab list renders the placeholder card, never the
/// bare hand pane with nothing in its place.
#[tokio::test]
async fn test_a_settled_tab_shows_the_placeholder_card() {
    let (app, pool, code, alice, _bob, _alice_id, _bob_id) = lc_action_rig().await;

    let mut st = lc_state(&pool, &code).await;
    st.beat = Beat::Draw;
    st.players[0].tabs = vec![]; // alice, seat 0 — settled, pre-Deal
    let game_id = lc_game_id(&pool, &code).await;
    drinkinggame::db::set_game_state(&pool, game_id, &st.to_json()).await;

    let alice_hand = body_string(get_hand(&app, &alice, &code).await).await;
    assert!(alice_hand.contains("data-tab-settled"), "{alice_hand}");
    assert!(alice_hand.contains("TAB SETTLED"), "{alice_hand}");
    assert!(!alice_hand.contains("data-tab=\""), "{alice_hand}");
}

// -------------------------------------------------------------
// Last Call (Plan I Task 4): the react and haunt routes — the Reveal beat's
// response window, and decision I3's grace extension.
//
// Rendering note: Plan I Task 5 (the reaction/haunt chips on the public
// screen and mini table) has not landed as of this task — `lc_render.rs`
// has no reader for `PublicView::reactions`/`haunts` yet (grep confirms
// it), the same gap `hand_seq`'s own doc comment above already flags
// ("used where a test cares that an action bumped `seq` but has no other
// observable effect yet (Task 4 renders the rest)"). So where the brief's
// test comments assert on literal chip text (e.g. "the frame contains
// 'Not So Fast'"), these tests instead assert on the PROJECTED state
// (`PublicView`/`LastCallState` fields `public_view()` already exposes
// verbatim per I9/I10) plus the frame's publish kind (`lcpublic` vs
// `lctick`) — the same public data Task 5's chip will render from, just
// without the HTML text to grep for yet.
// -------------------------------------------------------------

/// Shared rig: alice(seat0)/Beer, bob(seat1)/Cider register at Draw; cara
/// (seat2) is left `Status::Eliminated` with a cleared hand — the ghost the
/// haunt tests need. Bob's hand additionally holds `cider-08` ("Not So
/// Fast, Friend", a Reaction card, Cider deck) so the react tests never
/// have to route through a live shoe draw to get one. alice's `beer-02`
/// (Atk, targets one, Beer deck) arms -> targets bob (seat 1) -> locks; bob
/// locks nothing armed; `advance_beat()` walks Lock -> Reveal, producing
/// exactly one `Play` (`order_key` 1) aimed at bob — the play every
/// react/haunt call in this section answers. `deadline_ms` is the caller's
/// own choice of `beat_deadline_ms` to persist over the rig's own arm
/// (already-expired for the race-loser test, near-expiry for the grace
/// test, comfortably open for the rest).
async fn lc_reveal_rig(
    deadline_ms: i64,
) -> (
    Router,
    sqlx::SqlitePool,
    String, // code
    String, // alice cookie
    String, // bob cookie
    String, // cara cookie
    i64,    // alice id
    i64,    // bob id
    i64,    // cara id
) {
    let (app, pool) = test_app_with_pool().await;
    let alice = login(&app, "alice", "1234").await;
    let bob = login(&app, "bob", "5678").await;
    let cara = login(&app, "cara", "9999").await;
    let code = create_room(&app, &alice).await;
    room_page_html(&app, &bob, &code).await;
    room_page_html(&app, &cara, &code).await;
    post_form(&app, &alice, &format!("/room/{code}/lastcall/start"), "").await;

    let alice_id = drinkinggame::db::get_player_by_name(&pool, "alice")
        .await
        .unwrap()
        .id;
    let bob_id = drinkinggame::db::get_player_by_name(&pool, "bob")
        .await
        .unwrap()
        .id;
    let cara_id = drinkinggame::db::get_player_by_name(&pool, "cara")
        .await
        .unwrap()
        .id;

    let mut st = LastCallState::new(
        vec![
            (alice_id, "alice".into()),
            (bob_id, "bob".into()),
            (cara_id, "cara".into()),
        ],
        7,
    );
    st.set_vessel(alice_id, Deck::Beer, "can").unwrap();
    st.set_vessel(bob_id, Deck::Cider, "bottle").unwrap();
    st.players[2].status = drinkinggame::last_call::Status::Eliminated; // cara: ghost
    st.players[2].hand.clear();
    st.players[1]
        .hand
        .push(drinkinggame::lc_cards::card_by_id("cider-08").unwrap());
    st.beat = Beat::Lock;
    st.arm(alice_id, "beer-02").unwrap();
    st.set_target(alice_id, "beer-02", Some(1)).unwrap(); // bob's seat
    st.lock_in(alice_id).unwrap();
    st.lock_in(bob_id).unwrap(); // bob locks nothing armed
    st.advance_beat().unwrap(); // Lock -> Reveal
    assert_eq!(st.beat, Beat::Reveal);
    assert_eq!(st.plays.len(), 1);
    assert_eq!(st.plays[0].order_key, 1);
    st.beat_deadline_ms = Some(deadline_ms);

    let game_id = lc_game_id(&pool, &code).await;
    drinkinggame::db::set_game_state(&pool, game_id, &st.to_json()).await;

    (app, pool, code, alice, bob, cara, alice_id, bob_id, cara_id)
}

/// §3.3: an unplayed reaction is hand state, never transport-visible to
/// anyone else. Once played it's public immediately (I9), not merely at
/// resolve — the assertion below reads that off `persist_and_broadcast_lc`'s
/// full publish (vs. the tick-only path arm/disarm/target take) and off the
/// state's own `reactions`/`public_view().reactions` projection, since
/// Task 5's chip text isn't rendered yet (see the section comment above).
#[tokio::test]
async fn test_lc_react_is_private_until_played_then_public() {
    // A deadline inside the grace floor (4s < REACT_GRACE_SECS's 10) so the
    // extension this test also checks is an observable change, not a no-op
    // against an already-longer window — same reasoning as
    // `test_a_response_extends_the_window`'s 3s rig. Still comfortably
    // longer than this test's own request round-trips, so the 1 Hz ticker
    // has no real chance to win a race against it.
    let (app, pool, code, alice, bob, _cara, _alice_id, bob_id, _cara_id) =
        lc_reveal_rig(unix_ms_now() + 4_000).await;

    // Bob's unplayed reaction never reaches alice's hand fragment.
    let alice_hand = body_string(get_hand(&app, &alice, &code).await).await;
    assert!(!alice_hand.contains("cider-08"), "{alice_hand}");
    assert!(!alice_hand.contains("Not So Fast"), "{alice_hand}");

    let res = get(&app, &format!("/room/{code}/sse")).await;
    let mut body = res.into_body().into_data_stream();
    read_sse_until(&mut body, "event: lcpublic").await; // drain the snapshot
    let before = lc_state(&pool, &code).await.beat_deadline_ms.unwrap();

    let res = post_form(
        &app,
        &bob,
        &format!("/room/{code}/lastcall/react"),
        "card_id=cider-08&play=1",
    )
    .await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    // Full publish, not a bare tick (I2/I3: a played reaction is public,
    // same broadcast policy as lock/draw).
    let seen = read_sse_until(&mut body, "event: lcpublic").await;
    assert!(seen.contains("event: lcpublic"), "{seen}");
    // The banner's live timer (`lc_render::beat_timer_live`) already rides
    // every `lcpublic` frame (Plan E, decision E10) — its `data-deadline-ms`
    // is a genuine public-surface trace of decision I3's extension, visible
    // today even without Task 5's chip.
    let marker = "data-deadline-ms=\"";
    let start = seen.find(marker).unwrap() + marker.len();
    let rest = &seen[start..];
    let after_deadline: i64 = rest[..rest.find('"').unwrap()].parse().unwrap();
    assert!(after_deadline > before, "{after_deadline} vs {before}");

    // Bob's card left his hand...
    let bob_hand = body_string(get_hand(&app, &bob, &code).await).await;
    assert!(!bob_hand.contains("cider-08"), "{bob_hand}");
    // ...and the play is now part of the state `public_view()` projects
    // verbatim (I9) — the same data Task 5's chip will render from.
    let after = lc_state(&pool, &code).await;
    assert_eq!(after.reactions.len(), 1);
    assert_eq!(after.reactions[0].card.id, "cider-08");
    assert_eq!(after.reactions[0].source_seat, 1); // bob
    assert_eq!(after.reactions[0].answers, 1);
    assert_eq!(after.public_view().reactions.len(), 1);
    let _ = bob_id;
}

/// The race this task's Class C exists for, from the outside: subscribe SSE
/// (this is what puts the room in `active_rooms()`, so the 1 Hz ticker
/// notices the already-expired deadline), then WAIT for the ticker's own
/// chain to actually land (`data-beat="draw"` — Reveal -> Resolve ->
/// resolve() rolls straight into round 2's Draw, since nothing was locked
/// to answer). Only once that frame has arrived does the route fire:
/// `lc_react_handler` relocks, `load_lc` reloads the now-post-resolution
/// state, and `play_reaction`'s own `WrongBeat` guard refuses before
/// `extend_response_window` is ever reached — a clean 409, never a
/// resolve-time surprise.
#[tokio::test]
async fn test_lc_react_that_loses_the_race_gets_a_409() {
    let (app, _pool, code, _alice, bob, _cara, _alice_id, _bob_id, _cara_id) =
        lc_reveal_rig(unix_ms_now() - 2_000).await; // already expired

    let res = get(&app, &format!("/room/{code}/sse")).await;
    let mut body = res.into_body().into_data_stream();
    read_sse_until(&mut body, r#"data-beat="draw""#).await;

    let res = post_form(
        &app,
        &bob,
        &format!("/room/{code}/lastcall/react"),
        "card_id=cider-08&play=1",
    )
    .await;
    assert_eq!(res.status(), StatusCode::CONFLICT); // WrongBeat -> OutOfTurn
}

/// Decision I3's arithmetic at the route level (the in-crate
/// `test_extend_response_window_never_shortens`/
/// `test_a_react_extension_and_the_ticker_do_not_race` pin the helper and
/// the race directly; this pins that the route actually calls it under the
/// guard it persists under). No SSE subscription here deliberately: with
/// only 3s left on the clock, putting the room in `active_rooms()` would
/// hand the 1 Hz ticker a real chance to win the race and advance the beat
/// before the POST lands, which is exactly the OTHER test's scenario — this
/// one wants the extension to land clean.
#[tokio::test]
async fn test_a_response_extends_the_window() {
    let (app, pool, code, _alice, bob, _cara, _alice_id, _bob_id, _cara_id) =
        lc_reveal_rig(unix_ms_now() + 3_000).await;
    let recorded = lc_state(&pool, &code).await.beat_deadline_ms.unwrap();

    let res = post_form(
        &app,
        &bob,
        &format!("/room/{code}/lastcall/react"),
        "card_id=cider-08&play=1",
    )
    .await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    let after = lc_state(&pool, &code).await.beat_deadline_ms.unwrap();
    assert!(after > recorded, "{after} vs {recorded}");
    assert!(
        after >= unix_ms_now() + 8_000, // grace 10s minus slack
        "{after} vs now+8000={}",
        unix_ms_now() + 8_000
    );
}

/// I10: haunt is the sole ghost action, `Status::Eliminated` only — an
/// Alive seat gets `NotAGhost` (403), same as a non-member gets the route's
/// own `member_room` guard (403), and a second vote from the same ghost
/// this round gets `AlreadyHaunted` (409). The successful vote is public
/// immediately, same rendering caveat as the react test above: asserted on
/// the projected `haunts` field and the full-publish marker rather than a
/// chip's HTML text.
#[tokio::test]
async fn test_lc_haunt_is_for_ghosts_only_and_lands_public() {
    let (app, pool, code, alice, _bob, cara, _alice_id, _bob_id, cara_id) =
        lc_reveal_rig(unix_ms_now() + 20_000).await;
    let carol = login(&app, "carol", "2222").await; // never joins the room

    // Alive seat: NotAGhost -> NotYourCall.
    let res = post_form(
        &app,
        &alice,
        &format!("/room/{code}/lastcall/haunt"),
        "play=1",
    )
    .await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // Non-member: the route's own member_room guard, same 403.
    let res = post_form(
        &app,
        &carol,
        &format!("/room/{code}/lastcall/haunt"),
        "play=1",
    )
    .await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    let res = get(&app, &format!("/room/{code}/sse")).await;
    let mut body = res.into_body().into_data_stream();
    read_sse_until(&mut body, "event: lcpublic").await; // drain the snapshot

    // cara: eliminated, a ghost — the vote lands.
    let res = post_form(
        &app,
        &cara,
        &format!("/room/{code}/lastcall/haunt"),
        "play=1",
    )
    .await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    let seen = read_sse_until(&mut body, "event: lcpublic").await;
    assert!(seen.contains("event: lcpublic"), "{seen}"); // full publish, not a tick

    let after = lc_state(&pool, &code).await;
    assert_eq!(after.haunts.len(), 1);
    assert_eq!(after.haunts[0].seat, 2); // cara
    assert_eq!(after.haunts[0].play, 1);
    assert_eq!(after.public_view().haunts.len(), 1);

    // Same ghost, same round, again: AlreadyHaunted -> OutOfTurn.
    let res = post_form(
        &app,
        &cara,
        &format!("/room/{code}/lastcall/haunt"),
        "play=1",
    )
    .await;
    assert_eq!(res.status(), StatusCode::CONFLICT);
    let _ = cara_id;
}
