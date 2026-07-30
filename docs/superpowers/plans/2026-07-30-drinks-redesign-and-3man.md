# Drinks Redesign + 3 Man Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the `/drinks` UI with the phone-first three-tab redesign (Ring of Fire on the new shell, QR join, house rules, sounds/emotes, end-of-night summary) and add a second game, 3 Man (two dice, live seating, doubles give-away flow).

**Architecture:** Keep the existing loop end-to-end: handlers mutate SQLite → `render.rs` rebuilds HTML fragments server-side → per-room SSE hub broadcasts → clients swap `innerHTML`. Broadcast fragments are identical for every viewer; anything per-viewer is resolved client-side by one generic `personalize()` pass driven by `data-*` attributes. 3 Man is a pure serde state machine (`three_man.rs`) snapshotted into `games.state_json`, mutated under a per-room `tokio::sync::Mutex`.

**Tech Stack:** Rust, Axum 0.8, sqlx 0.8 (SQLite), Askama 0.15, vanilla JS + htmx, SSE. New server dep: `qrcode` (SVG render). No new client deps.

**Spec:** `docs/superpowers/specs/2026-07-30-drinks-redesign-and-3man-design.md` — read it before any task.
**Visual source of truth:** `docs/superpowers/specs/prototypes/redesign.html` and `docs/superpowers/specs/prototypes/3man.html`. These are Claude-Design exports: all styling is inline `style="…"` attributes and `{{ }}`/`sc-if`/`sc-for` are template markers. Translate inline styles into `game.css` classes; never copy the marker syntax.

## Global Constraints

- Palette (exact): background `#0b0910`, surface `#17141f`, chip `#262232`, violet `#b48ef7`, amber `#ffb570`, red `#f7768e`, text `#f2eef8`, text-dim `#cdc6dd`, text-faint `#8d87a0`, card face `#f7f3ea`, on-violet ink `#191624`, hairline `rgba(242,238,248,.09)`.
- Fonts: Archivo (weights 500–900, display) + Space Grotesk (400–700, UI), **self-hosted** woff2 under `drinkinggame/assets/fonts/`, embedded with `include_bytes!`, served from `/assets/fonts/*`. No third-party requests from any page.
- Vanilla JS only. No new client-side dependencies (htmx stays).
- SQL lives in `db.rs` only; fragments in `render.rs` only; route handlers orchestrate.
- Broadcast fragments are identical for every viewer. Per-viewer differences ONLY via the `personalize()` data-attribute contract (Task 4 defines it; every later task obeys it).
- Drinks templates do NOT extend the portfolio's `base.html` (recorded exception).
- All `/drinks` timestamps are ISO8601 TEXT (SQLite convention already in place).
- Workspace commands from repo root: `cargo test -p drinkinggame`, `cargo clippy`, `cargo fmt --check`. Every task ends with all three green.
- Migration rule: `ALTER TABLE ADD COLUMN` is not idempotent in SQLite — guard every ALTER with a `PRAGMA table_info` check in `run_migrations()`.
- Auto-logged verdict drinks never fire emotes; self-logged drinks/shots always do.
- Sounds play only on the tapping phone; emote floats come only from the SSE broadcast (no local float — that would double).

## Fragment/DOM contract (used by Tasks 4–13 — read before any of them)

Container ids in `room.html` (Task 7) and `screen.html` (Task 8):

| id | filled by | SSE event |
|---|---|---|
| `#game-panel` | phone GAME tab | `game` |
| `#standings-list` | phone STANDINGS tab `<ol>` rows | `leaderboard` |
| `#room-panel` | phone ROOM/TABLE tab | `room` (main part) |
| `#topbar-strip` | shell top bar | `room` (copied from fragment's `<template data-topbar>`) |
| `#screen-panel` | big-screen left pane incl. footer strip | `screen` |
| `#emote-layer` | float-up glyphs | `emote` |
| `#game-error` | 4xx fragment surface | htmx `responseError` |

Data-attribute contract (server emits, `personalize()` consumes):

- `data-show-player="ID"` — element hidden unless viewer id == ID (server renders it with the `hidden` attribute already set; personalize un-hides).
- `data-hide-player="ID"` — element hidden when viewer id == ID.
- `data-me-text="…"` + `data-player-id="ID"` on the same element — personalize swaps `textContent` to the me-text when ID is the viewer.
- Standings rows: `<li data-player-id data-drinks data-shots data-rank>`; personalize adds `.lb-me` to the viewer's row and copies counts into every `[data-my-drinks]` / `[data-my-shots]` element (thumb-bar labels, idle stat card).
- Animation keying: fragment root carries `data-anim-key="…"`; elements to animate carry `data-anim="flip|pop|tumble"`. Client compares the incoming key with the last seen key per SSE event name and, only when changed, adds class `anim-a` / `anim-b` (alternating) to every `[data-anim]` element. Keys — Ring of Fire `"{draw_count}-{spend_count}"`; 3 Man `"{seq}"`.
- `room` fragment root carries `data-mode="idle|ring_of_fire|three_man"`; client renames tab 3 (ROOM ↔ TABLE) from it.
- 3 Man seat strip (Task 11): container carries `data-order="id,id,…"` `data-roller="id"` `data-three-man="id"`; a client helper derives the viewer's exposure line ("You're on their left — a 7 is yours.").

---

## Phase 1 — shell + Ring of Fire

### Task 1: Migration 003, model + db-layer groundwork

**Files:**
- Create: `drinkinggame/migrations/003_shell_and_three_man.sql`
- Modify: `drinkinggame/src/db.rs`, `drinkinggame/src/models.rs`, `drinkinggame/src/error.rs`, `drinkinggame/src/game.rs` (call sites only), `drinkinggame/tests/http.rs` (seed helpers only)
- Test: inline `#[cfg(test)]` in `db.rs`

**Interfaces (produces — later tasks rely on these exact signatures):**
```rust
// models.rs — extended / new
pub struct Game { id, room_id, rules_json, deck_order, created_at, ended_at, pub kind: String, pub state_json: Option<String> }
pub struct DrawRow { id, player_id, player_name, card_index, spent_at, pub rank: i64 }
pub struct LeaderboardRow { pub id: i64, pub name: String, pub drinks: i64, pub shots: i64 }
pub struct HouseRule { pub id: i64, pub draw_id: i64, pub player_id: i64, pub player_name: String, pub text: String }
pub struct RoomMember { pub id: i64, pub name: String, pub joined_at: String }

// db.rs — new / changed
pub async fn start_game(pool, room_id, kind: &str, rules_json: &str, deck_order: &str, state_json: Option<&str>) -> Result<i64, GameError>
pub async fn insert_draw(pool, game_id, player_id, deck_ranks: &[u8]) -> Result<i64, GameError> // writes rank = deck_ranks[index] on insert
pub async fn set_game_state(pool, game_id, state_json: &str)
pub async fn insert_events_bulk(pool, room_id, player_id, kind: &str, n: u32)
pub async fn room_members(pool, room_id) -> Vec<RoomMember>          // ORDER BY joined_at, player_id
pub async fn insert_house_rule(pool, game_id, draw_id, player_id, text: &str) -> Result<i64, sqlx::Error> // Err = UNIQUE(draw_id) violation
pub async fn house_rules(pool, game_id) -> Vec<HouseRule>            // joins players for name, ORDER BY id
pub async fn king_count(pool, game_id) -> i64                        // draws WHERE rank = 13
pub async fn last_king_drawer(pool, game_id) -> Option<String>       // name of latest rank-13 drawer
pub async fn lifetime_nights(pool, player_id) -> i64                 // COUNT(DISTINCT room_id) FROM room_players
pub async fn lifetime_kings(pool, player_id) -> i64                  // COUNT(*) FROM game_draws WHERE player_id=? AND rank=13
// end_room and end_inactive_rooms now ALSO end any active game in those rooms
```

- [ ] **Step 1: Write the migration file**

`003_shell_and_three_man.sql` contains only the idempotent parts (the ALTERs live in code):

```sql
-- Shell + 3 Man. House rules typed after drawing a Jack; draw_id UNIQUE
-- makes it one rule per Jack, server-verifiable.
CREATE TABLE IF NOT EXISTS game_house_rules (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    game_id INTEGER NOT NULL REFERENCES games(id),
    draw_id INTEGER NOT NULL UNIQUE REFERENCES game_draws(id),
    player_id INTEGER NOT NULL REFERENCES players(id),
    text TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_house_rules_game ON game_house_rules(game_id);
```

- [ ] **Step 2: Write failing db tests**

Add to `db.rs` tests (they won't compile yet — that counts as failing):

```rust
#[tokio::test]
async fn test_migration_003_adds_columns_and_is_idempotent() {
    let pool = test_pool().await;
    run_migrations(&pool).await; // second run must not error
    // Columns exist with defaults.
    let g = seed_game(&pool).await; // updated helper, see step 4
    let game = get_active_game(&pool, g.0).await.unwrap();
    assert_eq!(game.kind, "ring_of_fire");
    assert!(game.state_json.is_none());
}

#[tokio::test]
async fn test_rank_backfill_is_idempotent_and_correct() {
    let pool = test_pool().await;
    let (_room, game, alice, _bob) = seed_game(&pool).await;
    let deck = crate::cards::parse_deck(&get_active_game_deck(&pool, game).await);
    let ranks: Vec<u8> = deck.iter().map(|c| c.rank).collect();
    insert_draw(&pool, game, alice, &ranks).await.unwrap();
    // Simulate a pre-003 row: null out the rank, then re-run migrations.
    sqlx::query("UPDATE game_draws SET rank = NULL").execute(&pool).await.unwrap();
    run_migrations(&pool).await;
    run_migrations(&pool).await; // idempotent
    let draws = get_draws(&pool, game).await;
    assert_eq!(draws[0].rank, deck[0].rank as i64);
}

#[tokio::test]
async fn test_house_rule_one_per_draw() {
    let pool = test_pool().await;
    let (room, game, alice, _bob) = seed_game(&pool).await;
    let deck = crate::cards::parse_deck(&get_active_game(&pool, room).await.unwrap().deck_order);
    let ranks: Vec<u8> = deck.iter().map(|c| c.rank).collect();
    insert_draw(&pool, game, alice, &ranks).await.unwrap();
    let draw_id = get_draws(&pool, game).await[0].id;
    assert!(insert_house_rule(&pool, game, draw_id, alice, "no names").await.is_ok());
    assert!(insert_house_rule(&pool, game, draw_id, alice, "again").await.is_err());
    let rules = house_rules(&pool, game).await;
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].player_name, "alice");
}

#[tokio::test]
async fn test_lifetime_nights_and_kings() {
    // two rooms joined => nights 2; rig a game whose deck puts a King at
    // index 0, draw it => kings 1 (see start_rigged pattern in tests/http.rs)
}

#[tokio::test]
async fn test_insert_events_bulk_counts_rows() {
    let pool = test_pool().await;
    let (room, alice, _bob) = seed_room_with_players(&pool).await;
    insert_events_bulk(&pool, room, alice, "drink", 4).await;
    let lb = leaderboard(&pool, room).await;
    assert_eq!(lb.iter().find(|r| r.name == "alice").unwrap().drinks, 4);
}

#[tokio::test]
async fn test_end_room_ends_active_game() {
    let pool = test_pool().await;
    let (room, game, _a, _b) = seed_game(&pool).await;
    end_room(&pool, room).await;
    let row: (Option<String>,) = sqlx::query_as("SELECT ended_at FROM games WHERE id = ?1")
        .bind(game).fetch_one(&pool).await.unwrap();
    assert!(row.0.is_some());
}

#[tokio::test]
async fn test_end_inactive_rooms_ends_their_games() { /* backdate room like test_end_inactive_rooms, assert games.ended_at set */ }
```

- [ ] **Step 3: Run to verify failure** — `cargo test -p drinkinggame db::` → compile errors for missing fns. Expected.

- [ ] **Step 4: Implement**

In `run_migrations()` after 002, execute 003, then code-guarded ALTERs + backfill:

```rust
async fn column_exists(pool: &DbPool, table: &str, column: &str) -> bool {
    let cols: Vec<(String,)> =
        sqlx::query_as(&format!("SELECT name FROM pragma_table_info('{table}')"))
            .fetch_all(pool).await.expect("pragma_table_info failed");
    cols.iter().any(|(c,)| c == column)
}
// in run_migrations():
if !column_exists(pool, "games", "kind").await {
    sqlx::query("ALTER TABLE games ADD COLUMN kind TEXT NOT NULL DEFAULT 'ring_of_fire'").execute(pool).await.expect("003 kind");
}
if !column_exists(pool, "games", "state_json").await {
    sqlx::query("ALTER TABLE games ADD COLUMN state_json TEXT").execute(pool).await.expect("003 state_json");
}
if !column_exists(pool, "game_draws", "rank").await {
    sqlx::query("ALTER TABLE game_draws ADD COLUMN rank INTEGER").execute(pool).await.expect("003 rank");
}
// Rank backfill — WHERE rank IS NULL makes it idempotent.
let games: Vec<(i64, String)> = sqlx::query_as(
    "SELECT id, deck_order FROM games WHERE deck_order != '' AND id IN
     (SELECT DISTINCT game_id FROM game_draws WHERE rank IS NULL)")
    .fetch_all(pool).await.expect("backfill scan");
for (gid, deck_order) in games {
    let deck = crate::cards::parse_deck(&deck_order);
    let draws: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT id, card_index FROM game_draws WHERE game_id = ?1 AND rank IS NULL")
        .bind(gid).fetch_all(pool).await.expect("backfill read");
    for (id, idx) in draws {
        sqlx::query("UPDATE game_draws SET rank = ?1 WHERE id = ?2")
            .bind(deck[idx as usize].rank as i64).bind(id)
            .execute(pool).await.expect("backfill write");
    }
}
```

Then: extend the two model structs and `LeaderboardRow` (add `p.id AS id` to the leaderboard SELECT, `gd.rank` to `get_draws`, `kind`/`state_json` to game SELECTs). `start_game` inserts kind + state_json. `insert_draw` takes `deck_ranks: &[u8]` and binds `deck_ranks[next_index]` as rank inside the existing retry loop. `end_room` and `end_inactive_rooms` each additionally run `UPDATE games SET ended_at = datetime('now') WHERE room_id = ?1 AND ended_at IS NULL` (for `end_inactive_rooms`, loop the returned ids). Reword `GameError::NoActiveGame` to `#[error("no game is running")]`. Implement the new query fns per the Interfaces block. Mechanically update call sites: `game.rs` `start_game(..., "ring_of_fire", &preset.rules_json, &deck, None)` and `insert_draw(&state.pool, game.id, player.id, &ranks)` where `let ranks: Vec<u8> = cards::parse_deck(&game.deck_order).iter().map(|c| c.rank).collect();`; update `tests/http.rs` seed helpers the same way. Update existing db tests that call the old signatures.

- [ ] **Step 5: Run** — `cargo test -p drinkinggame` all green; `cargo clippy` and `cargo fmt --check` clean.
- [ ] **Step 6: Commit** — `git add -A && git commit -m "feat(drinks): migration 003 — game kind/state, draw ranks, house rules, lifetime stats"`

### Task 2: Hub message kinds, capacity, per-room locks

**Files:**
- Modify: `drinkinggame/src/hub.rs`, `drinkinggame/src/lib.rs`, `drinkinggame/src/routes.rs` (end handler + sweep call sites)
- Test: inline in `hub.rs` and `lib.rs`

**Interfaces (produces):**
```rust
pub enum RoomMessage { Leaderboard(String), Game(String), Screen(String), Room(String), Emote(String), Ended }
// GameState gains:
pub struct GameState { pool, hub, base_path, pub locks: RoomLocks }
#[derive(Clone, Default)] pub struct RoomLocks { /* Arc<std::sync::Mutex<HashMap<i64, Arc<tokio::sync::Mutex<()>>>>> */ }
impl RoomLocks {
    pub fn for_room(&self, room_id: i64) -> Arc<tokio::sync::Mutex<()>>;
    pub fn remove(&self, room_id: i64);
}
```

- [ ] **Step 1: Failing tests** — in `hub.rs`: extend `test_subscribe_publish_remove` to round-trip `Screen`/`Room`/`Emote` variants; new `test_channel_capacity_is_128` (send 100 messages with a live receiver, assert none lost: `for _ in 0..100 { rx.try_recv().unwrap? }` — actually assert the 100th arrives). In `lib.rs`: `test_room_locks_serialize_access` — two tasks each `lock().await`, increment a shared counter with a `tokio::time::sleep(10ms)` inside the critical section, assert no interleaving (final ordering vector is `[start,end,start,end]`).
- [ ] **Step 2: Run to verify failure** — `cargo test -p drinkinggame hub` → compile error (missing variants). Expected.
- [ ] **Step 3: Implement** — add the three variants; `broadcast::channel(128)`; add `RoomLocks` (std Mutex map guarding `Arc<tokio::sync::Mutex<()>>` entries, `or_default().clone()` inside, never `.await` while holding the std lock) in `lib.rs`; add `locks: RoomLocks::default()` to `GameState` construction; call `state.locks.remove(room.id)` wherever `state.hub.remove(room.id)` is called (end handler, cleanup sweep).
- [ ] **Step 4: Run** — tests green, clippy, fmt.
- [ ] **Step 5: Commit** — `feat(drinks): hub screen/room/emote kinds, capacity 128, per-room lock map`

### Task 3: Static assets — self-hosted fonts, sounds route, QR helper

**Files:**
- Create: `drinkinggame/assets/fonts/*.woff2` (9 files), `drinks-sounds/README.md`
- Modify: `drinkinggame/Cargo.toml`, `drinkinggame/src/routes.rs`, `drinkinggame/src/render.rs`, `.env.example`
- Test: `tests/http.rs` + `render.rs` inline

- [ ] **Step 1: Download and commit fonts**

```bash
cd drinkinggame/assets && mkdir -p fonts && cd fonts
curl -sL -o archivo.zip "https://gwfh.mranftl.com/api/fonts/archivo?download=zip&subsets=latin&formats=woff2&variants=500,600,700,800,900"
curl -sL -o grotesk.zip "https://gwfh.mranftl.com/api/fonts/space-grotesk?download=zip&subsets=latin&formats=woff2&variants=regular,500,600,700"
unzip -o archivo.zip && unzip -o grotesk.zip && rm -f *.zip
# Normalize to stable names the route handler can match on:
for f in archivo-*-500.woff2; do mv "$f" archivo-500.woff2; done   # repeat for 600,700,800,900
for f in space-grotesk-*regular*.woff2; do mv "$f" space-grotesk-400.woff2; done  # and 500,600,700
ls  # expect exactly: archivo-{500,600,700,800,900}.woff2 space-grotesk-{400,500,600,700}.woff2
```
(If gwfh is unreachable, fetch the woff2 URLs from `https://fonts.googleapis.com/css2?family=Archivo:wght@500..900&family=Space+Grotesk:wght@400..700` with a Chrome User-Agent and download those instead — same target filenames.)

- [ ] **Step 2: Failing tests**

`tests/http.rs`:
```rust
#[tokio::test]
async fn test_font_and_sound_routes() {
    let app = test_app().await;
    // Fonts embedded — always 200 with the right type.
    let res = get(&app, "/assets/fonts/archivo-800.woff2").await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.headers()["content-type"], "font/woff2");
    // Unknown font name → 404, no traversal.
    assert_eq!(get(&app, "/assets/fonts/../../etc/passwd").await.status(), StatusCode::NOT_FOUND);
    // Sounds: allowlisted name but no file on disk → 404 (drop-in dir ships empty).
    assert_eq!(get(&app, "/assets/sounds/drink.mp3").await.status(), StatusCode::NOT_FOUND);
    // Non-allowlisted name → 404 even if a file existed.
    assert_eq!(get(&app, "/assets/sounds/evil.sh").await.status(), StatusCode::NOT_FOUND);
}
```
`render.rs`:
```rust
#[test]
fn test_qr_svg_renders() {
    let svg = qr_svg("https://example.com/drinks/room/QK4M");
    assert!(svg.starts_with("<svg"));
    assert!(svg.contains("f2eef8")); // dark modules in text color
}
```

- [ ] **Step 3: Run to verify failure.**
- [ ] **Step 4: Implement**

`Cargo.toml`: `qrcode = { version = "0.14", default-features = false }`.

`render.rs`:
```rust
pub fn qr_svg(url: &str) -> String {
    use qrcode::render::svg;
    qrcode::QrCode::new(url.as_bytes())
        .expect("qr encode")
        .render::<svg::Color>()
        .quiet_zone(false)
        .min_dimensions(160, 160)
        .dark_color(svg::Color("#f2eef8"))
        .light_color(svg::Color("transparent"))
        .build()
}
```

`routes.rs`:
```rust
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
    ([(header::CONTENT_TYPE, "font/woff2"),
      (header::CACHE_CONTROL, "public, max-age=31536000, immutable")], bytes).into_response()
}

const SOUND_FILES: [&str; 6] = ["drink.mp3", "shot.mp3", "card-draw.mp3", "card-use.mp3", "dice-roll.mp3", "dice-give.mp3"];
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
```
Register `/assets/fonts/{name}` and `/assets/sounds/{name}`. Create `drinks-sounds/README.md` listing the six filenames and "drop mp3s here; missing files 404 and the client stays silent". Add `DRINKS_SOUNDS_DIR=` (with a comment) to `.env.example`.

- [ ] **Step 5: Run** — tests, clippy, fmt. Commit: `feat(drinks): self-hosted fonts, sound drop-in route, QR svg helper`

### Task 4: render.rs rewrite — Phase-1 fragments

**Files:**
- Modify: `drinkinggame/src/render.rs` (rewrite), `drinkinggame/src/game.rs` (view construction), `drinkinggame/tests/http.rs` (markup assertions)
- Test: inline in `render.rs`

This task defines all Phase-1 markup. Read the prototype `redesign.html` first: phone shell lines 40–260, big screen 263–378, login 380–395, start-or-join 396–470. Only the fragment content is built here; the shells (`room.html`, `screen.html`) come in Tasks 7–8, but they consume these exact classes — pull every visual value (spacing, radii, font shorthand) from the prototype's inline styles into semantic classes in Task 7's CSS. Class names used by fragments (contract for Task 7): `.announce`, `.deck-bar`, `.deck-fill`, `.deck-left`, `.hero-card`, `.card-big`, `.card-rank-tl`, `.card-suit-tl`, `.card-pip`, `.card-rank-br`, `.rule-kicker`, `.rule-title`, `.rule-text`, `.rule-input-row`, `.btn-primary`, `.btn-draw`, `.held-strip`, `.held-card`, `.held-face`, `.use-btn`, `.btn-ghost`, `.btn-danger`, `.stat-card`, `.stat-row`, `.start-card`, `.start-card-amber`, `.lb-row`, `.lb-me`, `.lb-rank`, `.lb-name`, `.lb-counts`, `.member-grid`, `.member-chip`, `.rules-list`, `.kings-fill`, `.summary-hero`, `.superla-grid`, `.superla-cell`, `.screen-hero`, `.screen-held`, `.screen-footer`, `.qr-box`.

**Interfaces (produces):**
```rust
pub struct GameView<'a> {
    pub base_path: &'a str, pub code: &'a str,
    pub current: Option<CurrentCard>,          // CurrentCard gains: pub drawer_id: i64, pub draw_id: i64, pub pending_rule: bool
    pub remaining: i64, pub held: Vec<HeldCardView>,
    pub counts: &'a [DrawCount], pub announcement: Option<String>,
    pub anim_key: String,                      // "{draws}-{spends}"
}
pub struct GameSummary { pub hardest: Option<(String, i64)>, pub most_shots: Option<(String, i64)>,
    pub room_total: i64, pub kings_cup: Option<String>, pub counts: Vec<DrawCount>, pub house_rules: Vec<HouseRule> }
pub struct RoomView<'a> { pub base_path: &'a str, pub code: &'a str, pub members: &'a [RoomMember],
    pub house_rules: &'a [HouseRule], pub kings: i64, pub mode: &'a str }

pub fn leaderboard_items(rows: &[LeaderboardRow]) -> String        // li rows with data-player-id/-drinks/-shots/-rank
pub fn game_idle_panel(base_path, code, presets: &[RulePreset]) -> String  // stat card ([data-my-drinks]/[data-my-shots]) + RoF start card
pub fn game_active_panel(view: &GameView) -> String
pub fn game_over_panel(s: &GameSummary) -> String
pub fn screen_panel_idle(code: &str) -> String                     // "Just drinking." + footer
pub fn screen_panel_active(view: &GameView, rules: &[HouseRule], kings: i64) -> String
pub fn screen_panel_over(s: &GameSummary) -> String
pub fn room_panel(view: &RoomView) -> String                       // starts with <template data-topbar>…</template>, root div carries data-mode
pub fn qr_svg(url: &str) -> String                                 // from Task 3
```

- [ ] **Step 1: Failing render tests** — replace the existing render test module wholesale:

```rust
#[test] fn test_leaderboard_rows_carry_data_attrs() {
    let rows = vec![LeaderboardRow { id: 7, name: "<x>".into(), drinks: 2, shots: 1 }];
    let html = leaderboard_items(&rows);
    assert!(html.contains(r#"data-player-id="7""#));
    assert!(html.contains(r#"data-drinks="2""#));
    assert!(html.contains(r#"data-rank="1""#));
    assert!(html.contains("&lt;x&gt;"));
}
#[test] fn test_active_panel_contract() {
    // build a GameView with a Jack current card, pending_rule=true, one held card
    let html = game_active_panel(&view);
    assert!(html.contains(r#"data-anim-key="3-1""#));
    assert!(html.contains(r#"data-anim="flip""#));
    // Jack input revealed only on the drawer's phone:
    assert!(html.contains(&format!(r#"data-show-player="{}" hidden"#, drawer_id)));
    assert!(html.contains("/drinks/room/QK4M/game/rule"));
    // USE button per personalization contract:
    assert!(html.contains(r#"data-show-player="2" hidden"#));
    assert!(html.contains("TAP TO DRAW"));
}
#[test] fn test_active_panel_non_jack_has_no_rule_input() { /* pending_rule=false → no rule form */ }
#[test] fn test_over_panel_superlatives() { /* hardest hit name, MOST DRAWS cell, King's Cup name, surviving house rules */ }
#[test] fn test_room_panel_topbar_and_mode() {
    let html = room_panel(&view);
    assert!(html.starts_with("<template data-topbar>"));
    assert!(html.contains(r#"data-mode="idle""#));
    assert!(html.contains("WHO"));            // who's here grid
    assert!(html.contains("OPEN BIG SCREEN"));
    assert!(html.contains("kings-fill"));
}
#[test] fn test_screen_panels() { /* idle contains "Just drinking."; active contains hero + HELD RIGHT NOW + footer; over contains "lost" */ }
#[test] fn test_idle_panel_has_stat_card_and_start() {
    // [data-my-drinks] placeholder, preset <select>, START button, presets link
}
```
Keep `test_escape`, `test_card_face_marks_red_suits` (update to the new `.card-big` markup), preset tests unchanged.

- [ ] **Step 2: Run to verify failure.**
- [ ] **Step 3: Implement the builders.** Structure each per the prototype regions listed above. Key details:
  - Card face: rank top-left + suit glyph under it + large center pip + rotated rank bottom-right (prototype lines 86–91), red suits get `.card-red`.
  - Active panel order: announcement (if any) → deck bar + `N LEFT` → hero card (`data-anim="flip"`, kicker "`{drawer}` DREW" with `data-me-text="YOU DREW" data-player-id="{drawer_id}"`) → Jack rule form when `pending_rule` (`<form hx-post=…/game/rule>` wrapped in `data-show-player="{drawer_id}" hidden`) → TAP TO DRAW (`.btn-draw`, subcaption "FREE FOR ALL · ANYONE CAN PULL", `data-sound="card-draw"`) → IN HAND strip (USE button inside `data-show-player="{holder_id}" hidden`, `data-sound="card-use"`) → End game early (`.btn-ghost`, `hx-confirm`).
  - Anim key on the active-panel root: `data-anim-key="{draws}-{spends}"` (the view's `anim_key`).
  - Over panel: "GAME OVER" header, HARDEST HIT card (name + draws), 2×2 superlatives grid (MOST DRAWS / MOST SHOTS / ROOM TOTAL / KING'S CUP), surviving house rules list, then the caller appends the idle panel below (unchanged pattern).
  - Room panel: `<template data-topbar>` containing member initial chips + "`N` here" (or "`N` at the table" when mode `three_man`) — Task 11 adds the 3 MAN chip; then room-code card with SHARE LINK (`data-share` button — client JS) and OPEN BIG SCREEN link (`{base}/room/{code}/screen`, `target="_blank"`), WHO'S HERE grid (dot = membership), HOUSE RULES list ("`name` · rule text", byline `data-me-text="your rule"`), King's Cup fill (`{kings} / 4` with 4 pips), End the night form (`onsubmit` confirm).
  - Screen active: display-scale hero card + rule, HELD RIGHT NOW strip, footer strip (King's Cup fill + house-rules one-liner). Screen over: "`{kings_cup or hardest}` lost." + superlatives grid. Screen idle: "Just drinking." + footer.
  - Update `game.rs::active_panel` to fill the new view fields (`drawer_id`, `draw_id`, `pending_rule` = rank 11 && no house rule for that draw yet, `anim_key`) — it may need `db::house_rules` to compute `pending_rule`.
  - Update `tests/http.rs` string assertions that reference removed markup ("Start Ring of Fire" → "START", "cards left" → "LEFT", "Tap to draw" → "TAP TO DRAW", `data-holder-id` → `data-show-player`). Run the suite and fix every mismatch — assertion updates only, no behavior edits.
- [ ] **Step 4: Run** — full suite, clippy, fmt.
- [ ] **Step 5: Commit** — `feat(drinks): redesign fragment builders — game/screen/room panels, superlatives, data-attr contract`

### Task 5: Wiring — broadcasts, Jack rule route, kind guards, emotes, SSE snapshot

**Files:**
- Modify: `drinkinggame/src/game.rs`, `drinkinggame/src/routes.rs`, `drinkinggame/src/error.rs`
- Test: `tests/http.rs`

**Interfaces (produces):**
```rust
// game.rs
pub async fn current_panel(state, room_id, code, announcement) -> String        // phone variant (kept name)
pub async fn current_screen_panel(state, room_id, code) -> String               // spectator variant
pub(crate) async fn broadcast_game(state, room_id, code, announcement: Option<String>)  // publishes Game AND Screen
pub(crate) async fn broadcast_room(state, room_id, code)                        // publishes Room
pub async fn rule_handler(...)                                                  // POST /room/{code}/game/rule, form { text }
// error.rs
GameError::WrongGameKind   // #[error("that action belongs to the other game")] → 409
GameError::NotYourCall     // #[error("that move isn't yours to make")] → 403
GameError::OutOfTurn       // #[error("someone beat you to it")] → 409
GameError::TooFewPlayers   // #[error("that game needs at least 2 players")] → 409
GameError::RuleTooLong     // #[error("rule must be 1–200 characters")] → 422
```

- [ ] **Step 1: Failing http tests**

```rust
#[tokio::test] async fn test_jack_rule_flow() {
    // start_rigged_game with a Jack at index 0 (reuse the rig helper: craft deck_order starting "JS,...")
    // draw as alice → POST /game/rule text=No names as alice → 204
    // room fragment (page reload) shows the rule; second POST for same draw → 409
    // POST /game/rule as bob (not the drawer) → 403
    // POST 201-char text → 422
}
#[tokio::test] async fn test_rule_rejected_when_latest_draw_not_jack() { /* rig non-jack top card → 409 */ }
#[tokio::test] async fn test_rof_routes_reject_three_man_games() {
    // insert a games row with kind='three_man' directly via sqlx
    // POST /game/draw, /game/spend, /game/rule, /game/end → all 409
}
#[tokio::test] async fn test_sse_snapshot_has_all_stateful_kinds() {
    // read the SSE stream: first four events are leaderboard, game, screen, room (any order ok — assert all four names appear before any broadcast)
}
#[tokio::test] async fn test_event_broadcasts_emote_and_room_join_broadcasts_room() {
    // subscribe via hub after joining; log drink → expect Emote("🍺") and a Leaderboard message; undo → no emote
    // second player GET /room/{code} → expect a Room message
}
#[tokio::test] async fn test_end_night_ends_game_and_room() { /* active game + POST /end → game ended, room ended */ }
```

- [ ] **Step 2: Run to verify failure.**
- [ ] **Step 3: Implement**
  - Add the five `GameError` variants with the statuses above.
  - `broadcast_game` renders phone + screen variants, publishes `RoomMessage::Game` + `RoomMessage::Screen`. Replace every `broadcast_panel` call. `broadcast_game_over` publishes `game_over_panel + idle` on `game` and `screen_panel_over` on `screen`, then `broadcast_room` (king fill reset next game).
  - Kind guard: in `draw_handler`, `spend_handler`, `end_game_handler`, `rule_handler` after `get_active_game`: `if game.kind != "ring_of_fire" { return GameError::WrongGameKind.into_response(); }`.
  - `rule_handler`: member_room → active game + kind guard → `let Some(last) = draws.last()` with `last.rank == 11` else `WrongGameKind`→ no: return `OutOfTurn` (latest draw isn't an unruled Jack); `last.player_id == player.id` else `NotYourCall`; trimmed text 1..=200 else `RuleTooLong`; `insert_house_rule` (UNIQUE err → `OutOfTurn`); `touch_room`; `broadcast_room` + `broadcast_game(…, Some(format!("{} made a rule", player.name)))`.
  - `log_event`: after the leaderboard broadcast, `state.hub.publish(room.id, RoomMessage::Emote(if kind=="drink" {"🍺"} else {"🥃"}.into()))`.
  - `room_page`: after `join_room`, `broadcast_room` (new members appear on everyone's ROOM tab).
  - Draw broadcasts also call `broadcast_room` when the drawn rank is 13 (king fill changes).
  - `sse_stream`: snapshot now emits four events — `leaderboard`, `game` (phone panel), `screen`, `room` — then maps all six `RoomMessage` variants to their event names (`Emote` → event `emote`, data = glyph). Kinds map 1:1; `ended` unchanged.
  - `end_room_handler`: publish `Ended`, `hub.remove`, `locks.remove` (Task 2 already covers locks — verify).
- [ ] **Step 4: Run** — suite, clippy, fmt.
- [ ] **Step 5: Commit** — `feat(drinks): dual-surface broadcasts, jack rules, kind guards, emotes, sse snapshot`

### Task 6: QR join round-trip (`next` param) + origin derivation

**Files:**
- Modify: `drinkinggame/src/auth.rs`, `drinkinggame/src/routes.rs`, `drinkinggame/templates/landing.html` (hidden field only — full restyle is Task 8), `deploy/nginx.conf`, `CLAUDE.md`
- Test: `tests/http.rs`, inline in `routes.rs`

**Interfaces (produces):**
```rust
// routes.rs
pub fn request_origin(headers: &axum::http::HeaderMap) -> String
// e.g. "https://example.com" — X-Forwarded-Proto override; falls back to
// "http" for Host starting with "localhost"/"127.", else "https".
fn valid_next(base_path: &str, next: &str) -> bool  // ^{base}/room/[A-Z]{4}$ (alphabet letters), no regex crate
// LoginForm gains: next: Option<String>; LandingTemplate gains: next: String
```

- [ ] **Step 1: Failing tests**

```rust
#[test] fn test_valid_next() {
    assert!(valid_next("/drinks", "/drinks/room/QKAM"));
    assert!(!valid_next("/drinks", "/drinks/room/qkam"));
    assert!(!valid_next("/drinks", "https://evil.example/x"));
    assert!(!valid_next("/drinks", "/drinks/room/QKAM/../admin"));
    assert!(!valid_next("", "/room/QKAM/extra"));
}
#[tokio::test] async fn test_unauthenticated_room_visit_redirects_with_next() {
    // GET /room/QKAM without a cookie → 303 to /?next=/room/QKAM
}
#[tokio::test] async fn test_login_honors_valid_next_and_ignores_bad_next() {
    // POST /login name/pin/next=/room/{code} → redirect to that room
    // POST /login with next=https://evil → redirect to /
}
#[test] fn test_request_origin() {
    // Host=example.com → https://example.com; Host=localhost:3001 → http://localhost:3001
    // X-Forwarded-Proto=https + Host=localhost → https://localhost
}
```

- [ ] **Step 2: Run to verify failure.**
- [ ] **Step 3: Implement**
  - `PlayerSession` rejection: when the request is a GET whose path starts with `/room/`, redirect to `{base}/?next={base}{path}` (percent-encoding not needed — validated charset); otherwise keep redirecting to `{base}/`.
  - `landing`: read `next` from `Query<HashMap<String,String>>`, pass through to the template only if `valid_next`; template renders `<input type="hidden" name="next" value="{{ next }}">` when non-empty.
  - `login`: on success redirect to `form.next` if `valid_next(&state.base_path, next)` else `{base}/`.
  - `valid_next`: strip prefix `{base}/room/`, require exactly 4 remaining chars, all in `rooms::CODE_ALPHABET`.
  - `request_origin` as specified; used by `screen_page` (Task 8) for the QR URL: `format!("{origin}{base}/room/{code}")`.
  - `deploy/nginx.conf`: add `proxy_set_header X-Forwarded-Proto $scheme;` to the `/drinks` proxy location(s). CLAUDE.md deployment section: note the header line must be added manually on the server (nginx config is not CI-deployed).
- [ ] **Step 4: Run** — suite, clippy, fmt.
- [ ] **Step 5: Commit** — `feat(drinks): QR join round-trip — next param, origin derivation, nginx forwarded-proto note`

### Task 7: game.css rewrite + room.html three-tab shell

**Files:**
- Modify: `drinkinggame/assets/game.css` (rewrite), `drinkinggame/templates/room.html` (rewrite)

No Rust changes. The shell consumes Task 4's fragments and the DOM contract verbatim.

- [ ] **Step 1: Rewrite `game.css`.** Sections (named comments, in order): `/* tokens */` (CSS custom properties for the Global Constraints palette), `/* fonts */` (nine `@font-face` blocks, `src:url("fonts/archivo-500.woff2") format("woff2")` — relative to `/assets/`, `font-display:swap`), `/* base */`, `/* keyframes */` (flipA/B, popA/B, livePulse, floatUp, floatUpBig, tumbleA/B — copy from prototype heads, lines 18–25 of each file), `/* anim hooks */` (`.anim-a[data-anim="flip"]{animation:flipA .45s cubic-bezier(.2,.8,.2,1)}` and the b/pop/tumble equivalents), `/* shell */` (top bar, tabs, thumb bar), `/* game tab */`, `/* standings */`, `/* room tab */`, `/* screen */`, `/* landing */`, `/* presets */`, `/* error */`. Extract concrete values from the prototypes' inline styles for every contract class listed in Task 4. Thumb-zone bottom bar is `position:fixed; bottom:0` with 44px+ targets; hero card animates only via the anim hooks.
- [ ] **Step 2: Rewrite `room.html`.** Structure: top bar (room-code pill + livePulse dot, `#topbar-strip`, mute toggle) → tab row (GAME / STANDINGS / ROOM buttons, client-side switching via `data-tab` + `.tab-active`) → three tab panes (`#game-panel` pre-filled with `{{ game_panel|safe }}`, `<ol id="standings-list">{{ leaderboard_items|safe }}</ol>`, `#room-panel` pre-filled with `{{ room_panel|safe }}` — **add `room_panel` to `RoomTemplate` and fill it in `room_page`**, a one-line Rust touch allowed here) → fixed bottom bar (+1 DRINK / +1 SHOT with `data-sound`, tonight counts via `[data-my-drinks]`/`[data-my-shots]`, UNDO) → `#emote-layer` → `#game-error`. Body keeps `data-player-id="{{ player_id }}"`.

The full page script (write exactly this, then extend):

```html
<script>
const BP = "{{ base_path }}", CODE = "{{ code }}";
const lastKeys = {}; let animFlip = false;
function personalize(root) {
  root = root || document;
  const me = document.body.dataset.playerId;
  root.querySelectorAll("[data-show-player]").forEach(el => { el.hidden = el.dataset.showPlayer !== me; });
  root.querySelectorAll("[data-hide-player]").forEach(el => { el.hidden = el.dataset.hidePlayer === me; });
  root.querySelectorAll("[data-me-text]").forEach(el => { if (el.dataset.playerId === me) el.textContent = el.dataset.meText; });
  const mine = document.querySelector('#standings-list [data-player-id="' + me + '"]');
  if (mine) {
    mine.classList.add("lb-me");
    document.querySelectorAll("[data-my-drinks]").forEach(el => el.textContent = mine.dataset.drinks);
    document.querySelectorAll("[data-my-shots]").forEach(el => el.textContent = mine.dataset.shots);
  }
  if (typeof exposureLine === "function") exposureLine(root); // Task 11
}
function swapPanel(id, evName, html) {
  const el = document.getElementById(id);
  el.innerHTML = html;
  htmx.process(el);
  const keyed = el.querySelector("[data-anim-key]");
  if (keyed) {
    const key = keyed.dataset.animKey;
    if (evName in lastKeys && lastKeys[evName] !== key) {
      animFlip = !animFlip;
      el.querySelectorAll("[data-anim]").forEach(a => a.classList.add(animFlip ? "anim-a" : "anim-b"));
    }
    lastKeys[evName] = key;
  }
  personalize(el);
}
const es = new EventSource(BP + "/room/" + CODE + "/sse");
es.addEventListener("leaderboard", e => { document.getElementById("standings-list").innerHTML = e.data; personalize(); });
es.addEventListener("game", e => swapPanel("game-panel", "game", e.data));
es.addEventListener("room", e => {
  const tpl = document.createElement("div");
  tpl.innerHTML = e.data;
  const strip = tpl.querySelector("template[data-topbar]");
  if (strip) document.getElementById("topbar-strip").innerHTML = strip.innerHTML;
  swapPanel("room-panel", "room", e.data);
  const mode = tpl.querySelector("[data-mode]")?.dataset.mode || "idle";
  document.querySelector('[data-tab="room"]').textContent = mode === "three_man" ? "TABLE" : "ROOM";
});
es.addEventListener("emote", e => {
  const s = document.createElement("span");
  s.className = "emote-float";
  s.textContent = e.data;
  s.style.left = (12 + Math.random() * 70) + "%";
  document.getElementById("emote-layer").appendChild(s);
  setTimeout(() => s.remove(), 1000);
});
es.addEventListener("ended", () => { es.close(); window.location = BP + "/"; });
// Sounds: tapping phone only, muted via one global localStorage key.
function muted() { return localStorage.getItem("drinks_muted") === "1"; }
document.addEventListener("click", ev => {
  const t = ev.target.closest("[data-sound]");
  if (t && !muted()) new Audio(BP + "/assets/sounds/" + t.dataset.sound + ".mp3").play().catch(() => {});
});
// Mute toggle + tab switching + share button:
document.getElementById("mute-btn").addEventListener("click", () => {
  localStorage.setItem("drinks_muted", muted() ? "0" : "1"); syncMute();
});
function syncMute() { document.getElementById("mute-btn").textContent = muted() ? "🔇" : "🔊"; }
document.querySelectorAll("[data-tab]").forEach(b => b.addEventListener("click", () => {
  document.querySelectorAll("[data-tab]").forEach(x => x.classList.toggle("tab-active", x === b));
  document.querySelectorAll(".tab-pane").forEach(p => p.hidden = p.dataset.pane !== b.dataset.tab);
}));
document.addEventListener("click", ev => {
  if (!ev.target.closest("[data-share]")) return;
  const url = location.origin + BP + "/room/" + CODE;
  if (navigator.share) navigator.share({ url }).catch(() => {});
  else navigator.clipboard.writeText(url);
});
document.body.addEventListener("htmx:responseError", e => {
  const el = document.getElementById("game-error");
  el.innerHTML = e.detail.xhr.responseText;
  setTimeout(() => { el.innerHTML = ""; }, 4000);
});
personalize(); syncMute();
</script>
```
(`room_page` in `routes.rs`: build `room_panel` via a new `game.rs` helper `current_room_panel(state, room, mode)` that fills `RoomView` from `db::room_members`/`house_rules`/`king_count` — add it here if Task 5 didn't already; `broadcast_room` uses the same helper.)

- [ ] **Step 3: Verify in a real browser.** `cargo run -p drinkinggame`, open `http://localhost:3001`, log in, start a night, open the room in a second window (different name). Check: tabs switch; both windows' standings update on +1 DRINK; emote floats on both; UNDO works; mute persists across reload; USE buttons only on the holder's phone; Jack draw shows the rule input only on the drawer's phone.
- [ ] **Step 4: Run** — `cargo test -p drinkinggame` (http tests still green), clippy, fmt.
- [ ] **Step 5: Commit** — `feat(drinks): redesigned css + three-tab room shell with personalize/anim/emote/sound client`

### Task 8: screen.html, landing.html, presets/error restyle

**Files:**
- Modify: `drinkinggame/templates/screen.html` (rewrite), `drinkinggame/templates/landing.html` (rewrite), `drinkinggame/templates/presets.html`, `drinkinggame/templates/preset_edit.html`, `drinkinggame/templates/error.html` (class touch-ups only), `drinkinggame/src/routes.rs` (screen/landing template fields)
- Test: `tests/http.rs`

- [ ] **Step 1: Failing tests** — update/extend: `test_screen_is_public` asserts the page contains `qr-box` and the join code; `test_landing_serves_login_form` asserts "LET'S GO"; logged-in landing shows lifetime nights + kings (`ScreenTemplate` unchanged fields + `qr_svg`; `LandingTemplate` gains `lifetime_nights: i64, lifetime_kings: i64, next: String`).
- [ ] **Step 2: Run to verify failure.**
- [ ] **Step 3: Implement.** `screen.html` per prototype lines 263–378: 1280×720-oriented grid, left `#screen-panel` (pre-filled with `current_screen_panel`), right pane JOIN header (code + `{{ qr_svg|safe }}` — `screen_page` computes `qr_svg(&format!("{}{}/room/{}", request_origin(&headers), base_path, code))`), `<ol id="standings-list">` scaled to fill. SSE listeners: `screen` → `#screen-panel` (with anim-key logic — reuse the same swapPanel snippet, trimmed), `leaderboard`, `emote` (same float snippet as room.html but class `emote-float-big` / `floatUpBig` — spec: ALL surfaces float from the broadcast), `ended` → redirect home. No personalize (spectator has no player id). `landing.html` per prototype lines 380–470: login card (name/PIN/LET'S GO, hidden `next`) and, when logged in, "EVENING `NAME`" + lifetime stat headline (drinks · shots · nights · King's Cups) + START A NIGHT + join-code form. `landing` handler fills nights/kings via the Task 1 queries. Presets/edit/error keep their structure; swap class names onto the new CSS sections.
- [ ] **Step 4: Browser check** — screen page next to two phone windows; QR encodes the room URL (scan or decode manually); draw cards and watch the screen hero + standings react.
- [ ] **Step 5: Run** — suite, clippy, fmt. Commit: `feat(drinks): spectator screen with QR, redesigned landing, restyled presets`

### Task 9: Phase-1 verification checkpoint

**Files:** none new — fixes only.

- [ ] **Step 1:** `cargo fmt --check && cargo clippy && cargo test` (workspace, from repo root) — quote output.
- [ ] **Step 2:** Full browser walkthrough per spec Verification: login → start night → second window joins via room URL while logged out (lands on login with `next`, arrives in the room) → Ring of Fire full game including Jack rule + King's Cup fill + game-over summary → end night returns both windows to landing. Screen window follows along throughout.
- [ ] **Step 3:** Fix anything found (each fix TDD'd where practical), re-run.
- [ ] **Step 4: Commit** — `test(drinks): phase 1 verification fixes` (or no-op).

---

## Phase 2 — 3 Man

### Task 10: `three_man.rs` engine (pure + serde)

**Files:**
- Create: `drinkinggame/src/three_man.rs`
- Modify: `drinkinggame/src/lib.rs` (add `pub mod three_man;`)
- Test: inline

**Interfaces (produces — exact; Tasks 11–13 depend on every name here):**
```rust
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase { Ready, Rolled, HandOff, Assign, Gifts }
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum GiveMode { Both, Split }
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Call { pub player_id: i64, pub amount: u8, pub reason: String }
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Gift { pub player_id: i64, pub dice_count: u8, pub values: Option<Vec<u8>> }
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct DoubleState { pub value: u8, pub owner: i64, pub mode: Option<GiveMode>,
    pub slots: Vec<Option<i64>>, pub gifts: Vec<Gift>, pub payback: Option<u8> }
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ThreeManState {
    pub order: Vec<i64>, pub roller_idx: usize, pub three_man: i64, pub phase: Phase,
    pub dice: Option<(u8, u8)>, pub calls: Vec<Call>, pub double: Option<DoubleState>,
    pub pending_double: bool, pub handoff_from: Option<i64>, pub last_roller: Option<i64>,
    pub stale: bool, pub seq: u64,
}
#[derive(Debug, PartialEq)]
pub enum TmError { WrongPhase, BadTarget, TooFewPlayers }

impl ThreeManState {
    pub fn new(members: Vec<i64>, starter: i64) -> Self;      // rotate so starter is index 0; starter is 3 Man
    pub fn roller(&self) -> i64;
    pub fn left_of(&self, idx: usize) -> i64;                 // order[(idx+1) % len] — "next to roll"
    pub fn right_of(&self, idx: usize) -> i64;
    pub fn roll(&mut self, d1: u8, d2: u8) -> Result<(), TmError>;
    pub fn give_three_man(&mut self, target: i64) -> Result<(), TmError>;   // HandOff only
    pub fn set_mode(&mut self, mode: GiveMode) -> Result<(), TmError>;
    pub fn pick_target(&mut self, slot: usize, player: i64) -> Result<(), TmError>;
    pub fn clear_slot(&mut self, slot: usize) -> Result<(), TmError>;
    pub fn send(&mut self) -> Result<(), TmError>;
    pub fn gift_roll(&mut self, slot: usize, values: Vec<u8>) -> Result<u8, TmError>; // returns total the victim drinks
    pub fn gifts_complete(&self) -> bool;
    pub fn pass(&mut self) -> Result<(), TmError>;
    pub fn move_seat(&mut self, player: i64, delta: i64) -> Result<(), TmError>;
    pub fn set_three_man(&mut self, player: i64) -> Result<(), TmError>;    // table-tab reassign, any phase
    pub fn add_player(&mut self, player: i64);                              // mid-game join, idempotent
    pub fn to_json(&self) -> String;
    pub fn from_json(s: &str) -> Self;                                      // expect() — own serialization only
}
```
Spec deviation (documented): `handoff_note: Option<String>` → `handoff_from: Option<i64>` and `payback: Option<String>` → `payback: Option<u8>` — the engine stays name-free; render composes display text from ids. `seq` (added) increments on every `roll`/`gift_roll`/`pass` and feeds the `data-anim-key`.

- [ ] **Step 1: Write failing unit tests** — the whole battery, engine-only:

```rust
fn st3() -> ThreeManState { ThreeManState::new(vec![1, 2, 3], 1) }

#[test] fn test_new_rotates_starter_to_front() {
    let s = ThreeManState::new(vec![7, 8, 9], 8);
    assert_eq!(s.order, vec![8, 9, 7]);
    assert_eq!((s.roller(), s.three_man, s.phase), (8, 8, Phase::Ready));
}
#[test] fn test_plain_roll() { // 2+4: nobody drinks
    let mut s = st3(); s.set_three_man(2).unwrap();
    s.roll(2, 4).unwrap();
    assert_eq!(s.phase, Phase::Rolled);
    assert!(s.calls.is_empty());
    assert_eq!(s.seq, 1);
}
#[test] fn test_single_three_hits_three_man() {
    let mut s = st3(); s.set_three_man(2).unwrap();
    s.roll(3, 5).unwrap();
    assert_eq!(s.calls, vec![Call { player_id: 2, amount: 1, reason: "a 3 on the dice".into() }]);
}
#[test] fn test_three_total_counts() { // 1+2
    let mut s = st3(); s.set_three_man(2).unwrap();
    s.roll(1, 2).unwrap();
    assert_eq!(s.calls[0].amount, 1);
}
#[test] fn test_double_threes_count_each_and_fire_doubles() {
    let mut s = st3(); s.set_three_man(2).unwrap();
    s.roll(3, 3).unwrap();
    assert_eq!(s.calls[0], Call { player_id: 2, amount: 2, reason: "two 3s on the dice".into() });
    assert_eq!(s.phase, Phase::Assign);
    assert_eq!(s.double.as_ref().unwrap().value, 3);
}
#[test] fn test_seven_nine_eleven() {
    let mut s = st3(); s.set_three_man(3).unwrap();
    s.roll(3, 4).unwrap(); // 7 AND a three
    let ids: Vec<i64> = s.calls.iter().map(|c| c.player_id).collect();
    assert!(ids.contains(&3));            // three_man for the 3
    assert!(ids.contains(&s.left_of(0))); // 7 → left
    s = st3(); s.set_three_man(3).unwrap();
    s.roll(4, 5).unwrap();                // 9 → right
    assert_eq!(s.calls[0].player_id, s.right_of(0));
    s = st3(); s.set_three_man(3).unwrap();
    s.roll(5, 6).unwrap();                // 11 → roller
    assert_eq!(s.calls[0].player_id, 1);
}
#[test] fn test_two_player_left_equals_right() {
    let mut s = ThreeManState::new(vec![1, 2], 1);
    assert_eq!(s.left_of(0), 2); assert_eq!(s.right_of(0), 2);
    s.set_three_man(2).unwrap();
    s.roll(4, 3).unwrap(); // 7 → the other player
    assert!(s.calls.iter().any(|c| c.player_id == 2));
}
#[test] fn test_three_man_rolls_three_goes_to_handoff_no_drink() {
    let mut s = st3(); // three_man == roller == 1
    s.roll(3, 6).unwrap();
    assert_eq!(s.phase, Phase::HandOff);
    assert!(s.calls.is_empty());
    assert!(!s.pending_double);
}
#[test] fn test_handoff_with_pending_double() {
    let mut s = st3();
    s.roll(3, 3).unwrap();
    assert_eq!(s.phase, Phase::HandOff);
    assert!(s.pending_double);
    assert_eq!(s.give_three_man(1), Err(TmError::BadTarget)); // not to yourself
    s.give_three_man(3).unwrap();
    assert_eq!((s.three_man, s.handoff_from, s.phase), (3, Some(1), Phase::Assign));
}
#[test] fn test_handoff_without_double_goes_to_rolled() {
    let mut s = st3();
    s.roll(3, 6).unwrap();
    s.give_three_man(2).unwrap();
    assert_eq!(s.phase, Phase::Rolled);
}
#[test] fn test_assign_both_flow_and_payback() {
    let mut s = st3(); s.set_three_man(2).unwrap();
    s.roll(4, 4).unwrap();
    s.set_mode(GiveMode::Both).unwrap();
    assert_eq!(s.double.as_ref().unwrap().slots.len(), 1);
    assert_eq!(s.pick_target(0, 1), Err(TmError::BadTarget)); // owner excluded
    s.pick_target(0, 3).unwrap();
    s.send().unwrap();
    assert_eq!(s.phase, Phase::Gifts);
    assert_eq!(s.double.as_ref().unwrap().gifts[0].dice_count, 2);
    let total = s.gift_roll(0, vec![4, 2]).unwrap();  // a gifted die == double value 4
    assert_eq!(total, 6);
    assert_eq!(s.double.as_ref().unwrap().payback, Some(6)); // owner drinks combined total
    assert!(s.gifts_complete());
}
#[test] fn test_assign_split_flow_no_payback() {
    let mut s = st3(); s.set_three_man(2).unwrap();
    s.roll(5, 5).unwrap();
    s.set_mode(GiveMode::Split).unwrap();
    s.pick_target(0, 2).unwrap();
    assert_eq!(s.pick_target(1, 2), Err(TmError::BadTarget)); // distinct slots
    s.pick_target(1, 3).unwrap();
    s.send().unwrap();
    s.gift_roll(0, vec![2]).unwrap();
    assert!(!s.gifts_complete());
    s.gift_roll(1, vec![6]).unwrap();
    assert!(s.gifts_complete());
    assert_eq!(s.double.as_ref().unwrap().payback, None); // no 5 rolled
}
#[test] fn test_split_rejected_under_three_players() {
    let mut s = ThreeManState::new(vec![1, 2], 2);
    s.roll(2, 2).unwrap();
    assert_eq!(s.set_mode(GiveMode::Split), Err(TmError::TooFewPlayers));
    s.set_mode(GiveMode::Both).unwrap();
}
#[test] fn test_clear_slot_and_resend() { /* pick, clear, send fails on empty slot, re-pick, send ok */ }
#[test] fn test_pass_advances_left_and_wraps_and_marks_stale() {
    let mut s = st3(); s.set_three_man(2).unwrap();
    s.roll(2, 4).unwrap();
    s.pass().unwrap();
    assert_eq!((s.roller(), s.last_roller, s.stale, s.phase), (2, Some(1), true, Phase::Ready));
    s.roll(2, 4).unwrap(); s.pass().unwrap();
    s.roll(2, 4).unwrap(); s.pass().unwrap();
    assert_eq!(s.roller(), 1); // wrapped
}
#[test] fn test_pass_blocked_until_gifts_done() {
    let mut s = st3(); s.set_three_man(2).unwrap();
    s.roll(6, 6).unwrap(); s.set_mode(GiveMode::Both).unwrap();
    s.pick_target(0, 2).unwrap(); s.send().unwrap();
    assert_eq!(s.pass(), Err(TmError::WrongPhase));
    s.gift_roll(0, vec![1, 2]).unwrap();
    s.pass().unwrap();
}
#[test] fn test_wrong_phase_everything() {
    let mut s = st3();
    assert_eq!(s.pass(), Err(TmError::WrongPhase));
    assert_eq!(s.give_three_man(2), Err(TmError::WrongPhase));
    assert_eq!(s.set_mode(GiveMode::Both), Err(TmError::WrongPhase));
    assert_eq!(s.gift_roll(0, vec![1]), Err(TmError::WrongPhase));
    s.roll(2, 4).unwrap();
    assert_eq!(s.roll(2, 4), Err(TmError::WrongPhase)); // roll from Rolled
}
#[test] fn test_move_seat_preserves_roller_and_wraps() {
    let mut s = st3();
    s.roll(2, 4).unwrap(); s.pass().unwrap(); // roller is now player 2
    s.move_seat(2, -1).unwrap(); // swaps 2 to front
    assert_eq!(s.roller(), 2);   // same player still rolling
    s.move_seat(1, -1).unwrap(); // wrap: index 0 - 1 swaps with last
}
#[test] fn test_table_reassign_any_time_resolves_handoff() {
    let mut s = st3();
    s.roll(3, 3).unwrap(); // HandOff + pending double
    s.set_three_man(3).unwrap();
    assert_eq!(s.phase, Phase::Assign); // handoff resolved by the table pick
    assert_eq!(s.set_three_man(99), Err(TmError::BadTarget));
}
#[test] fn test_add_player_appends_once() {
    let mut s = st3();
    s.add_player(4); s.add_player(4);
    assert_eq!(s.order, vec![1, 2, 3, 4]);
}
#[test] fn test_json_roundtrip() {
    let mut s = st3(); s.roll(3, 4).unwrap();
    assert_eq!(ThreeManState::from_json(&s.to_json()), s);
}
```

- [ ] **Step 2: Run to verify failure** (module doesn't exist).
- [ ] **Step 3: Implement.** Core of `roll`:

```rust
pub fn roll(&mut self, d1: u8, d2: u8) -> Result<(), TmError> {
    if self.phase != Phase::Ready { return Err(TmError::WrongPhase); }
    let sum = d1 + d2;
    let roller = self.roller();
    self.dice = Some((d1, d2));
    self.calls.clear();
    self.double = None;
    self.pending_double = false;
    self.handoff_from = None;
    self.stale = false;
    self.seq += 1;

    let threes = u8::from(d1 == 3) + u8::from(d2 == 3) + u8::from(sum == 3);
    let mut handoff = false;
    if threes > 0 {
        if roller == self.three_man {
            handoff = true; // no drink — the title moves instead
        } else {
            let reason = match threes { 1 => "a 3 on the dice", 2 => "two 3s on the dice", _ => "3s everywhere" };
            self.calls.push(Call { player_id: self.three_man, amount: threes, reason: reason.into() });
        }
    }
    match sum {
        7 => self.calls.push(Call { player_id: self.left_of(self.roller_idx), amount: 1, reason: "7 — left of the roller".into() }),
        9 => self.calls.push(Call { player_id: self.right_of(self.roller_idx), amount: 1, reason: "9 — right of the roller".into() }),
        11 => self.calls.push(Call { player_id: roller, amount: 1, reason: "11 — the roller".into() }),
        _ => {}
    }
    if d1 == d2 {
        self.double = Some(DoubleState { value: d1, owner: roller, mode: None, slots: vec![], gifts: vec![], payback: None });
    }
    self.phase = if handoff {
        self.pending_double = d1 == d2;
        Phase::HandOff
    } else if d1 == d2 {
        Phase::Assign
    } else {
        Phase::Rolled
    };
    Ok(())
}
```
`give_three_man`: HandOff-only; target in `order`, `target != self.three_man`; sets `handoff_from = Some(self.three_man)`, `three_man = target`, phase → `Assign` if `pending_double` else `Rolled`. `set_mode`: Assign-only, Split needs `order.len() >= 3`; sets slots `vec![None]`/`vec![None, None]`, clears gifts. `pick_target`: Assign, mode set, slot in range, player in order, `player != double.owner`, player not in another slot. `send`: all slots `Some` → gifts (`dice_count` 2 for Both, 1 for Split), phase `Gifts`. `gift_roll`: Gifts, slot valid, not yet rolled, `values.len() == dice_count`; `seq += 1`; on completion, `payback = all_values.contains(&value).then(|| all_values.sum())`. `pass`: from `Rolled` or (`Gifts` && `gifts_complete()`); `last_roller`, advance `roller_idx` `(i+1)%len`, `stale = true`, `seq += 1`, phase `Ready` (dice/calls/double kept for the stale render). `move_seat`: find index, `j = (i + delta).rem_euclid(len)`, remember roller id, swap, re-find `roller_idx`. `set_three_man`: target in order; if `phase == HandOff`, resolve it exactly like `give_three_man` (prevents a dead-lock if the picker's phone dies).
- [ ] **Step 4: Run** — all engine tests green, clippy, fmt.
- [ ] **Step 5: Commit** — `feat(drinks): 3 man engine — pure state machine with serde snapshot`

### Task 11: 3 Man fragments + client exposure line

**Files:**
- Modify: `drinkinggame/src/render.rs`, `drinkinggame/assets/game.css` (3-man section), `drinkinggame/templates/room.html` (add `exposureLine`)
- Test: inline in `render.rs`

Read prototype `3man.html`: phone 40–333, big screen 337–518. New CSS classes: `.turn-banner`, `.seat-strip`, `.seat`, `.seat-tag`, `.seat-rolling`, `.seat-left7`, `.seat-right9`, `.seat-3man`, `.verdict-card`, `.die`, `.die-pip`, `.call-row`, `.nobody-box`, `.handoff-panel`, `.assign-panel`, `.mode-btn`, `.slot-grid`, `.gift-row`, `.payback-banner`, `.stale`, `.seating-list`, `.seat-move`, `.rules-ref`, `.tm-chip`.

**Interfaces (produces):**
```rust
pub struct TmView<'a> {
    pub base_path: &'a str, pub code: &'a str,
    pub st: &'a crate::three_man::ThreeManState,
    pub names: &'a std::collections::HashMap<i64, String>,
}
pub fn dice_html(d1: u8, d2: u8) -> String                  // two .die pip grids, data-anim="tumble"
pub fn tm_phone_panel(v: &TmView) -> String                 // GAME tab, root data-anim-key="{seq}"
pub fn tm_screen_panel(v: &TmView) -> String                // big-screen left pane + bottom seat strip
pub fn tm_seating_html(v: &TmView) -> String                // TABLE-tab seating list + rules reference (room_panel embeds it)
// room_panel: RoomView gains pub seating: Option<String> (pre-rendered by tm_seating_html) and renders the 3 MAN chip + "N at the table" in the topbar template when mode == "three_man"
// leaderboard_items gains: pub fn leaderboard_items_tm(rows, three_man: Option<i64>) — adds a "3 MAN" badge span to the holder's row (plain leaderboard_items delegates with None)
```

- [ ] **Step 1: Failing render tests**

```rust
fn tm_view_fixture() -> (ThreeManState, HashMap<i64, String>) { /* 3 players alice/bob/cara, three_man bob */ }

#[test] fn test_tm_phone_ready_state() {
    // phase Ready: ROLL THE DICE button visible to everyone (any member can
    // trigger), attributed via turn banner: element with data-me-text="YOUR TURN"
    // data-player-id="{roller}"; seat strip data-order/-roller/-three-man attrs;
    // PASS absent in Ready phase.
    let html = tm_phone_panel(&v);
    assert!(html.contains(r#"data-order="1,2,3""#));
    assert!(html.contains(r#"data-anim-key="0""#));
    assert!(html.contains("/tm/roll"));
    assert!(!html.contains("/tm/pass"));
}
#[test] fn test_tm_phone_rolled_verdict_and_pass() {
    // after roll(3,4): dice pips, big sum 7, call rows "bob drinks 1" with
    // data-me-text="You drink 1", PASS TO <name> button posting /tm/pass
}
#[test] fn test_tm_phone_nobody_drinks_box() { /* roll(2,4) → .nobody-box */ }
#[test] fn test_tm_handoff_picker_only_on_roller_phone() {
    // roll(3,6) as three_man → picker grid wrapped in data-show-player="{roller}" hidden;
    // spectator banner wrapped in data-hide-player="{roller}"
}
#[test] fn test_tm_assign_owner_only_and_split_hidden_at_two_players() {
    // Assign: mode buttons inside data-show-player="{owner}"; with 2 players the SPLIT button is absent
}
#[test] fn test_tm_gifts_rows_and_payback() {
    // Gifts: one ROLL button per pending gift (no data-show-player — any phone),
    // posts /tm/gift-roll with slot; after all rolled + payback → .payback-banner with owner name
}
#[test] fn test_tm_stale_verdict_dimmed() { /* after pass(): .stale on verdict, "LAST ROLL · alice" */ }
#[test] fn test_tm_screen_waiting_only_before_first_roll() {
    // dice None → "WAITING ON alice"; dice Some + stale → dimmed verdict, no waiting pane
}
#[test] fn test_tm_seating_and_topbar_chip() {
    // tm_seating_html: ↑/↓ forms per row (/tm/seat), per-row 3 MAN assign (/tm/three-man),
    // rules reference cards; room_panel with mode three_man + seating → "3 MAN" chip and "at the table" in topbar template
}
#[test] fn test_leaderboard_tm_badge() { /* holder row contains "3 MAN" */ }
#[test] fn test_dice_pips() { assert_eq!(dice_html(5, 2).matches("die-pip").count(), 7); }
```

- [ ] **Step 2: Run to verify failure.**
- [ ] **Step 3: Implement.** Phone panel composition by phase (all in one builder, matching prototype):
  - Always: turn banner ("YOUR TURN" via me-text, else "`{roller}` IS UP"), seat strip (one tag per seat, precedence ROLLING > ←7 > 9→ > 3 MAN; caption "LEFT = NEXT TO ROLL"; data attrs per contract).
  - Ready: ROLL THE DICE (`hx-post=…/tm/roll`, `data-sound="dice-roll"`); if `stale`, the previous verdict card renders below with `.stale` + "LAST ROLL · `{last_roller}`".
  - Rolled: verdict card (dice pips `data-anim="tumble"`, sum, call rows / nobody box) + `PASS TO {left_of(roller_idx) name}` button.
  - HandOff: roller-only picker (grid of member buttons minus current 3 Man, `hx-post=…/tm/three-man`, `hx-vals` target id) + spectator banner "`{roller}` is picking the next 3 Man…".
  - Assign: owner-only mode choice (BOTH / SPLIT, hide SPLIT when `order.len() < 3`) → slot target grid (member buttons minus owner, picked slots shown with ✕ → `/tm/clear-slot`) → SEND THE DICE (`data-sound="dice-give"`, enabled when slots full); others see "`{owner}` is handing out the dice…".
  - Gifts: gift rows — each pending gift gets `ROLL {dice_count} DICE` (any phone, posts `/tm/gift-roll` slot); rolled gifts show values + "drinks `{total}`"; payback banner when set: "PAYBACK — `{owner}` drinks `{total}`"; when complete → PASS button.
  - End the game (ghost) at the bottom → `/tm/end` with `hx-confirm`.
  - Screen panel: 3 MAN header chip, giant dice + sum + reason headline, call rows at display scale, handoff/assign/gifts/payback banners, full-width bottom seat strip with the "7 hits the left · 9 hits the right · 11 hits the roller" caption; full-pane "WAITING ON `{roller}`" only when `st.dice.is_none()`.
  - `room.html` `exposureLine(root)`:
```js
function exposureLine(root) {
  const strip = (root || document).querySelector("[data-order]");
  const out = document.getElementById("exposure-line");
  if (!strip || !out) return;
  const me = document.body.dataset.playerId;
  const order = strip.dataset.order.split(",");
  const i = order.indexOf(strip.dataset.roller), mine = order.indexOf(me);
  if (i < 0 || mine < 0) { out.textContent = ""; return; }
  const n = order.length;
  const parts = [];
  if (mine === (i + 1) % n) parts.push("You're on their left — a 7 is yours.");
  if (mine === (i + n - 1) % n) parts.push("You're on their right — a 9 is yours.");
  if (me === strip.dataset.threeMan) parts.push("Any 3 is yours.");
  if (mine === i) parts.push("An 11 is yours.");
  out.textContent = parts.join(" ");
}
```
  (`tm_phone_panel` renders an empty `<p id="exposure-line">` under the ROLL button / roller banner.)
- [ ] **Step 4: Run** — suite, clippy, fmt.
- [ ] **Step 5: Commit** — `feat(drinks): 3 man fragments — seat strip, verdict, doubles flow, table tab, screen`

### Task 12: `/tm/start` + `/tm/end` + idle-panel start card + kind plumbing

**Files:**
- Create: `drinkinggame/src/tm_routes.rs` (module `tm_routes`, added to `lib.rs`)
- Modify: `drinkinggame/src/routes.rs` (register routes), `drinkinggame/src/game.rs` (panel dispatch by kind), `drinkinggame/src/render.rs` (idle panel gains the amber 3 Man start card)
- Test: `tests/http.rs`

**Interfaces (produces):**
```rust
// tm_routes.rs
pub(crate) struct TmCtx { pub room: Room, pub game: Game, pub st: ThreeManState }
pub(crate) async fn load_tm(state: &GameState, code: &str, player: &Player) -> Result<TmCtx, axum::response::Response>
// member_room → active game → kind == "three_man" else WrongGameKind → parse state
pub(crate) async fn persist_and_broadcast(state: &GameState, ctx: &TmCtx)     // set_game_state + game/screen/room(+topbar)/leaderboard-badge broadcasts
pub async fn tm_start_handler / tm_end_handler
// game.rs current_panel/current_screen_panel now dispatch: kind "three_man" → tm builders (via names map from room_members)
```

- [ ] **Step 1: Failing http tests**

```rust
#[tokio::test] async fn test_tm_start_seeds_order_and_renders_dice_ui() {
    // two players join; alice posts /tm/start → 204; room page contains "ROLL THE DICE"
    // and data-three-man = alice's id; games row has kind three_man, deck_order '' , rules_json ''
}
#[tokio::test] async fn test_tm_start_needs_two_players() { /* solo room → 409 */ }
#[tokio::test] async fn test_tm_start_conflicts_with_active_rof() { /* RoF running → 409 GameAlreadyActive */ }
#[tokio::test] async fn test_tm_routes_reject_rof_games() { /* RoF active, POST /tm/roll → 409 WrongGameKind */ }
#[tokio::test] async fn test_tm_end_broadcasts_summary_and_idle() { /* end → games.ended_at set, page shows both start cards */ }
#[tokio::test] async fn test_idle_panel_offers_both_games() { /* idle page: RoF start card + amber 3 MAN start card */ }
```

- [ ] **Step 2: Run to verify failure.**
- [ ] **Step 3: Implement.** `tm_start_handler`: member_room → `room_members` (`>= 2` else `TooFewPlayers`) → `ThreeManState::new(member_ids_by_joined_at, player.id)` → `db::start_game(pool, room.id, "three_man", "", "", Some(&st.to_json()))` (GameAlreadyActive races handled by the unique index) → touch, broadcast all. `tm_end_handler`: `load_tm` → `end_game` → broadcast `game_over_panel`-style 3 Man summary (reuse `GameSummary` minus kings: hardest = leaderboard-derived? — keep the RoF summary shape but hide the draws/kings cells when `deck_order` is empty) + idle panels + room. `current_panel`/`current_screen_panel`/`current_room_panel` dispatch on `game.kind` (three_man → TmView with `names` from `room_members`; RoomView.mode `three_man` + `seating`). Standings broadcast helper passes `three_man: Some(st.three_man)` while a 3 Man game is active. Idle panel: add the amber start card (`hx-post=…/tm/start`, one-line explainer "Two dice. 3s hit the 3 Man. Doubles hand out dice."). Register `/room/{code}/tm/start` + `/tm/end` in `routes.rs`.
- [ ] **Step 4: Run** — suite, clippy, fmt.
- [ ] **Step 5: Commit** — `feat(drinks): 3 man start/end, kind-dispatched panels, dual start cards`

### Task 13: 3 Man action routes — lock, gating, auto-log

**Files:**
- Modify: `drinkinggame/src/tm_routes.rs`, `drinkinggame/src/routes.rs` (registration + `room_page` mid-game join hook)
- Test: `tests/http.rs` (route-level happy paths; races covered by engine tests + lock test)

Routes (all POST under `/room/{code}`, all through `load_tm`, all executed while holding `state.locks.for_room(room.id).lock().await` across load → transition → persist, drink-event inserts included):

| route | form | actor gate | engine call |
|---|---|---|---|
| `/tm/roll` | — | any member | `roll(rand 1..=6, rand 1..=6)` |
| `/tm/three-man` | `target: i64` | roller if HandOff, else any | `give_three_man` / `set_three_man` (engine picks) |
| `/tm/mode` | `mode: "both"\|"split"` | `double.owner` else `NotYourCall` | `set_mode` |
| `/tm/target` | `slot: usize, target: i64` | owner | `pick_target` |
| `/tm/clear-slot` | `slot: usize` | owner | `clear_slot` |
| `/tm/send` | — | owner | `send` |
| `/tm/gift-roll` | `slot: usize` | any member | `gift_roll(slot, rand values)` |
| `/tm/pass` | — | any member | `pass` |
| `/tm/seat` | `target: i64, dir: i64 (±1)` | any member | `move_seat` |

`TmError` mapping: `WrongPhase` → `GameError::OutOfTurn` (409), `BadTarget` → `OutOfTurn`, `TooFewPlayers` → `TooFewPlayers` (409). Actor-gate failures → `NotYourCall` (403).

Auto-log inside the lock, after a successful transition, before persist+broadcast:
- `roll`: for each `Call` → `insert_events_bulk(pool, room_id, call.player_id, "drink", call.amount)`.
- `gift-roll`: `insert_events_bulk(victim, total)`; if this roll completed the gifts and `payback == Some(t)` → `insert_events_bulk(owner, t)`.
- One `broadcast_leaderboard` per handler invocation (not per row); NO emote broadcasts for auto-logged drinks.

- [ ] **Step 1: Failing http tests**

```rust
#[tokio::test] async fn test_tm_roll_any_member_and_autologs() {
    // bob (not the roller) POSTs /tm/roll → 204; loop-roll until a call fires
    // (state readable via games.state_json), then assert leaderboard counts grew by amount
}
#[tokio::test] async fn test_tm_roll_wrong_phase_409() { /* roll twice without pass → second 409 */ }
#[tokio::test] async fn test_tm_handoff_gating() {
    // rig state_json directly: phase HandOff, roller alice; bob posts /tm/three-man → 403; alice → 204
}
#[tokio::test] async fn test_tm_double_owner_gating() { /* rig Assign; non-owner /tm/mode → 403 */ }
#[tokio::test] async fn test_tm_gift_roll_autolog_and_payback() {
    // rig Gifts with slots filled; any member gift-rolls; victim's drinks grew by total;
    // rig values so payback fires → owner's drinks grew
}
#[tokio::test] async fn test_tm_seat_and_table_reassign_any_member() { /* /tm/seat + /tm/three-man outside HandOff as non-roller → 204 */ }
#[tokio::test] async fn test_tm_pass_after_rolled() { /* roll → pass → state_json roller advanced, stale true */ }
#[tokio::test] async fn test_midgame_join_appends_to_order() {
    // 3man running with 2 players; third player GETs /room/{code} → state_json order has 3 ids; room broadcast observed
}
#[tokio::test] async fn test_non_member_tm_403() { /* no-join player posts /tm/roll → 403 (redirect-free: has session) */ }
```
(Rigging helper: `UPDATE games SET state_json = ?` with a `ThreeManState` built in the test and `to_json()` — tests import the engine directly.)

- [ ] **Step 2: Run to verify failure.**
- [ ] **Step 3: Implement.** Handler skeleton (repeat per route with its gate + call):

```rust
pub async fn tm_roll_handler(State(state): State<GameState>, PlayerSession(player): PlayerSession,
                             Path(code): Path<String>) -> axum::response::Response {
    let lock = { // room id needed before locking: resolve room first, then lock, then re-load under the lock
        let Some(room) = db::get_open_room(&state.pool, &code.to_uppercase()).await else {
            return GameError::RoomNotFound.into_response();
        };
        state.locks.for_room(room.id)
    };
    let _guard = lock.lock().await;
    let mut ctx = match load_tm(&state, &code, &player).await { Ok(c) => c, Err(r) => return r };
    let (d1, d2) = { let mut rng = rand::thread_rng(); (rng.gen_range(1..=6), rng.gen_range(1..=6)) };
    if let Err(e) = ctx.st.roll(d1, d2) { return map_tm(e).into_response(); }
    let mut logged = false;
    for call in &ctx.st.calls {
        db::insert_events_bulk(&state.pool, ctx.room.id, call.player_id, "drink", call.amount as u32).await;
        logged = true;
    }
    db::set_game_state(&state.pool, ctx.game.id, &ctx.st.to_json()).await;
    db::touch_room(&state.pool, ctx.room.id).await;
    if logged { crate::routes::broadcast_leaderboard(&state, ctx.room.id).await; }
    persist_and_broadcast(&state, &ctx).await; // game + screen (+room when three_man/seat changed)
    StatusCode::NO_CONTENT.into_response()
}
```
`/tm/three-man` broadcasts `room` too (chip changes); `/tm/seat` broadcasts `room` + `game`/`screen`. `room_page` hook: after `join_room`, if the active game is `three_man` → take the room lock, reload state, `add_player`, persist, `broadcast_room` + `broadcast_game`. Register all routes.
- [ ] **Step 4: Run** — suite, clippy, fmt.
- [ ] **Step 5: Commit** — `feat(drinks): 3 man action routes — per-room lock, actor gating, auto-logged verdicts`

### Task 14: Phase-2 integration polish — cross-kind + snapshot + login round-trip coverage

**Files:**
- Modify: `drinkinggame/tests/http.rs`

Close the spec's Testing checklist gaps left after Tasks 12–13:

- [ ] **Step 1:** Add: `test_tm_sse_snapshot_includes_tm_panels` (SSE connect during a 3 Man game → `game` snapshot contains `data-order`, `room` snapshot contains the 3 MAN chip); `test_rof_full_deck_still_ends_after_redesign` (verify the 52nd-card flow still broadcasts final card then summary — update of the existing test if broken earlier); `test_undo_after_gift_is_per_row` (gift of 3 → victim UNDOes once → leaderboard shows 2 — documents the accepted caveat); `test_ending_room_with_tm_game_no_orphan` (end night mid-3-Man → no `ended_at IS NULL` game rows).
- [ ] **Step 2:** Run to verify the new tests fail only if behavior is missing; fix any real gaps they expose (TDD).
- [ ] **Step 3:** Full suite, clippy, fmt. Commit: `test(drinks): phase 2 integration coverage — snapshots, undo caveat, orphan guard`

### Task 15: Final verification, docs, deploy notes

**Files:**
- Modify: `CLAUDE.md` (drinks section: 3 Man, sounds dir, fonts, nginx header), `docs/design.md` if it references the old drinks UI

- [ ] **Step 1:** From repo root: `cargo fmt --check && cargo clippy && cargo test` — all green, quote output.
- [ ] **Step 2:** Full browser walkthrough (spec Verification section): login → start night → second window + screen window live → Ring of Fire full game incl. Jack rule + summary → start 3 Man → hand-off (roll 3s as the 3 Man — loop rolls until it fires or rig via sqlite CLI) → both doubles modes → payback → seat reorder mid-game → third player joins mid-game → end night. Both phone windows verify actor gating visually (picker on one phone, banner on the other).
- [ ] **Step 3:** Update `CLAUDE.md`: `/drinks` description (two games, presets, redesigned shell), `DRINKS_SOUNDS_DIR` env row, deploy note "add `proxy_set_header X-Forwarded-Proto $scheme;` to the server's nginx `/drinks` location (manual, not CI-deployed)", note that `drinks-sounds/` mp3s are drop-in on the server.
- [ ] **Step 4:** Commit: `docs: drinks redesign + 3 man shipped — CLAUDE.md and deploy notes`. Then follow superpowers:finishing-a-development-branch.

---

## Self-review notes (folded in)

- Spec coverage: decided-rules table → Task 10 (engine semantics) + Task 13 (gating); actor-gating table → Task 13; personalization contract → Task 4 contract + Task 7 `personalize()`; SSE protocol → Tasks 2/5; concurrency → Tasks 2/13; data model → Task 1; lifetime stats → Tasks 1/8; Phase-1 UI → Tasks 4–8; QR round-trip → Task 6; sounds → Tasks 3/7; 3 Man UI → Task 11; out-of-scope list untouched.
- Deviations from spec (all documented inline): `handoff_note`/`payback` typed as `Option<i64>`/`Option<u8>` (engine stays name-free); `/tm/send` route added (spec's route list omitted it but its flow requires it); `seq` field added to the state for animation keying; screen footer rides inside the `screen` fragment.
- Known-accepted caveats restated: UNDO tombstones latest event regardless of origin; mute key is global across rooms; presence dot = membership, not liveness.
