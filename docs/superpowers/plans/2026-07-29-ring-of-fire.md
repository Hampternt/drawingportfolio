# Ring of Fire Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A shared digital 52-card Ring of Fire game inside the existing `/drinks` rooms — any member draws, every screen (phones + spectator) updates live over the room's existing SSE channel, with server-side rule presets.

**Architecture:** All state is DB-backed in `drinkinggame.db` (new migration 002): a `games` row snapshots the preset rules and the full shuffled deck order; each draw is a `game_draws` row with a tombstone-style `spent_at` for held cards. The game panel is a server-rendered HTML fragment broadcast as a new `RoomMessage::Game(String)` variant over the existing per-room `tokio::sync::broadcast` hub; pages render current state server-side on load, SSE pushes full re-rendered panels (no client-side game state).

**Tech Stack:** Rust, Axum 0.8, sqlx 0.8 (SQLite, runtime-checked queries), Askama 0.15 (full pages only), HTMX, SSE, `rand` 0.8 (Fisher–Yates), `serde_json` (rules serialization), `thiserror`.

**Spec:** `docs/superpowers/specs/2026-07-29-ring-of-fire-design.md`

## Global Constraints

- **Workspace:** ALL work happens in the git worktree `/home/hampter/projects/drawingportfolio/.claude/worktrees/drinking-game-v1` on branch `worktree-drinking-game-v1`. Every file path below is relative to that directory. Do NOT touch the main checkout.
- SQL queries live in `drinkinggame/src/db.rs` ONLY — handlers call db functions, never raw SQL.
- HTML **fragments** are `format!` strings in `drinkinggame/src/render.rs` (crate convention, mirrors the portfolio's `post_card_html`). Full **pages** are Askama templates in `drinkinggame/templates/`.
- Timestamps are ISO8601 `TEXT` via SQLite `datetime('now')` — never UNIX integers.
- No client-side game state: pages render current state server-side; SSE only delivers freshly rendered full fragments.
- Migrations must be idempotent: `IF NOT EXISTS` guards, seed via `INSERT OR IGNORE`.
- New dependency: add `serde_json = "1"` to `drinkinggame/Cargo.toml` only. `rand = "0.8"` is ALREADY a dependency (room codes) — do not re-add or change its version.
- Tests: db-layer tests go in `#[cfg(test)] mod tests` blocks using the existing `test_pool()` (`max_connections(1)` on `sqlite::memory:` — a second connection would be a separate empty db). HTTP tests go in `drinkinggame/tests/http.rs` via `tower::ServiceExt`.
- Run tests from the worktree root: `cargo test -p drinkinggame`. Lint: `cargo clippy -p drinkinggame`. Format: `cargo fmt`.
- All user-provided strings passed through `render::html_escape` before interpolation into fragments.
- Success responses for HTMX mutation endpoints are `204 NO_CONTENT` (UI arrives via SSE); domain errors return the `GameError` HTML fragment with its status code.

## File Structure

| File | Responsibility |
|---|---|
| `drinkinggame/src/cards.rs` (new) | `Card`/`Suit` types, 52-card deck, Fisher–Yates shuffle, `"AS,2H,…"` (de)serialization |
| `drinkinggame/src/rules.rs` (new) | `RuleEntry`, the seeded Standard rules, rules-JSON (de)serialization |
| `drinkinggame/migrations/002_ring_of_fire.sql` (new) | `rule_presets`, `games`, `game_draws` tables + indexes |
| `drinkinggame/src/models.rs` (modify) | `RulePreset`, `Game`, `DrawRow`, `DrawCount` structs |
| `drinkinggame/src/db.rs` (modify) | migration 002 + Standard seed; preset CRUD; game lifecycle queries |
| `drinkinggame/src/error.rs` (modify) | 5 new `GameError` variants |
| `drinkinggame/src/hub.rs` (modify) | `RoomMessage::Game(String)` variant |
| `drinkinggame/src/render.rs` (modify) | card face + game panel fragment builders |
| `drinkinggame/src/game.rs` (new) | game route handlers (start/draw/spend/end) + shared panel builder |
| `drinkinggame/src/presets.rs` (new) | presets page handlers (list/create/edit/save/delete) |
| `drinkinggame/src/routes.rs` (modify) | register new routes; SSE game event; pass game panel to page templates |
| `drinkinggame/src/lib.rs` (modify) | declare new modules |
| `drinkinggame/templates/room.html` (modify) | game panel container, `data-player-id`, SSE listener, use-button reveal JS |
| `drinkinggame/templates/screen.html` (modify) | game panel container + SSE listener (spectator, no buttons) |
| `drinkinggame/templates/presets.html` (new) | preset list page |
| `drinkinggame/templates/preset_edit.html` (new) | 13-rank edit form |
| `drinkinggame/assets/game.css` (modify) | card faces, held strip, game panel, presets form |
| `drinkinggame/tests/http.rs` (modify) | integration tests for game flow, errors, presets |

---

### Task 1: Cards module

**Files:**
- Create: `drinkinggame/src/cards.rs`
- Modify: `drinkinggame/src/lib.rs` (add `pub mod cards;` to the module list, alphabetical order)

**Interfaces:**
- Consumes: `rand::seq::SliceRandom` (already a dependency).
- Produces (later tasks rely on these exact signatures):
  - `pub enum Suit { Spades, Hearts, Diamonds, Clubs }` with `pub fn glyph(self) -> &'static str`, `pub fn is_red(self) -> bool`, `pub fn code(self) -> char`, `pub fn from_code(c: char) -> Option<Suit>`
  - `pub struct Card { pub rank: u8, pub suit: Suit }` (rank `1..=13`) with `pub fn rank_label(self) -> &'static str`, `pub fn code(self) -> String`, `pub fn from_code(s: &str) -> Option<Card>`
  - `pub fn shuffled_deck() -> Vec<Card>` (all 52, shuffled)
  - `pub fn deck_to_string(deck: &[Card]) -> String` (`"AS,2H,10D,…"`)
  - `pub fn parse_deck(s: &str) -> Vec<Card>`

- [ ] **Step 1: Write the failing tests**

Create `drinkinggame/src/cards.rs` containing ONLY the test module for now:

```rust
//! Playing-card domain: 52-card deck, shuffle, and the compact
//! "AS,2H,10D" text encoding persisted in games.deck_order.

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn test_shuffled_deck_is_52_unique_cards() {
        let deck = shuffled_deck();
        assert_eq!(deck.len(), 52);
        let codes: HashSet<String> = deck.iter().map(|c| c.code()).collect();
        assert_eq!(codes.len(), 52);
    }

    #[test]
    fn test_deck_string_roundtrip() {
        let deck = shuffled_deck();
        let s = deck_to_string(&deck);
        assert_eq!(parse_deck(&s), deck);
        // ~150 bytes: 52 codes of 2-3 chars + 51 commas.
        assert!(s.len() < 200);
    }

    #[test]
    fn test_card_codes_and_labels() {
        let ace = Card { rank: 1, suit: Suit::Spades };
        assert_eq!(ace.code(), "AS");
        assert_eq!(ace.rank_label(), "A");
        let ten = Card { rank: 10, suit: Suit::Hearts };
        assert_eq!(ten.code(), "10H");
        assert_eq!(Card::from_code("10H"), Some(ten));
        assert_eq!(Card::from_code("QD"), Some(Card { rank: 12, suit: Suit::Diamonds }));
        assert_eq!(Card::from_code(""), None);
        assert_eq!(Card::from_code("XX"), None);
        assert_eq!(Card::from_code("14S"), None);
    }

    #[test]
    fn test_suit_properties() {
        assert!(Suit::Hearts.is_red());
        assert!(Suit::Diamonds.is_red());
        assert!(!Suit::Spades.is_red());
        assert!(!Suit::Clubs.is_red());
        assert_eq!(Suit::Spades.glyph(), "\u{2660}");
        assert_eq!(Suit::from_code('H'), Some(Suit::Hearts));
        assert_eq!(Suit::from_code('x'), None);
    }
}
```

Add `pub mod cards;` to `drinkinggame/src/lib.rs`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p drinkinggame cards -- --nocapture`
Expected: COMPILE ERROR — `shuffled_deck`, `Card`, `Suit` not found.

- [ ] **Step 3: Write the implementation**

Add above the test module in `drinkinggame/src/cards.rs`:

```rust
use rand::seq::SliceRandom;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Suit {
    Spades,
    Hearts,
    Diamonds,
    Clubs,
}

impl Suit {
    pub fn glyph(self) -> &'static str {
        match self {
            Suit::Spades => "\u{2660}",
            Suit::Hearts => "\u{2665}",
            Suit::Diamonds => "\u{2666}",
            Suit::Clubs => "\u{2663}",
        }
    }

    pub fn is_red(self) -> bool {
        matches!(self, Suit::Hearts | Suit::Diamonds)
    }

    pub fn code(self) -> char {
        match self {
            Suit::Spades => 'S',
            Suit::Hearts => 'H',
            Suit::Diamonds => 'D',
            Suit::Clubs => 'C',
        }
    }

    pub fn from_code(c: char) -> Option<Suit> {
        match c {
            'S' => Some(Suit::Spades),
            'H' => Some(Suit::Hearts),
            'D' => Some(Suit::Diamonds),
            'C' => Some(Suit::Clubs),
            _ => None,
        }
    }
}

/// rank is 1 (Ace) through 13 (King).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Card {
    pub rank: u8,
    pub suit: Suit,
}

const RANK_LABELS: [&str; 13] = [
    "A", "2", "3", "4", "5", "6", "7", "8", "9", "10", "J", "Q", "K",
];

impl Card {
    pub fn rank_label(self) -> &'static str {
        RANK_LABELS[(self.rank - 1) as usize]
    }

    pub fn code(self) -> String {
        format!("{}{}", self.rank_label(), self.suit.code())
    }

    pub fn from_code(s: &str) -> Option<Card> {
        if s.len() < 2 {
            return None;
        }
        let (rank_part, suit_part) = s.split_at(s.len() - 1);
        let suit = Suit::from_code(suit_part.chars().next()?)?;
        let rank = RANK_LABELS.iter().position(|&l| l == rank_part)? as u8 + 1;
        Some(Card { rank, suit })
    }
}

pub fn shuffled_deck() -> Vec<Card> {
    let mut deck: Vec<Card> = [Suit::Spades, Suit::Hearts, Suit::Diamonds, Suit::Clubs]
        .into_iter()
        .flat_map(|suit| (1..=13).map(move |rank| Card { rank, suit }))
        .collect();
    deck.shuffle(&mut rand::thread_rng());
    deck
}

pub fn deck_to_string(deck: &[Card]) -> String {
    deck.iter()
        .map(|c| c.code())
        .collect::<Vec<_>>()
        .join(",")
}

/// Panics on malformed input — deck strings only ever come from
/// deck_to_string via the games table, so corruption is a bug, not input.
pub fn parse_deck(s: &str) -> Vec<Card> {
    s.split(',')
        .map(|code| Card::from_code(code).expect("corrupt deck_order in db"))
        .collect()
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p drinkinggame cards -- --nocapture`
Expected: 4 passed.

- [ ] **Step 5: Lint, format, commit**

```bash
cargo clippy -p drinkinggame && cargo fmt
git add drinkinggame/src/cards.rs drinkinggame/src/lib.rs
git commit -m "feat(drinks): playing-card module with deck shuffle and text encoding"
```

---

### Task 2: Rules module + serde_json dependency

**Files:**
- Create: `drinkinggame/src/rules.rs`
- Modify: `drinkinggame/src/lib.rs` (add `pub mod rules;`)
- Modify: `drinkinggame/Cargo.toml` (add `serde_json = "1"` to `[dependencies]`)

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces:
  - `pub struct RuleEntry { pub rank: u8, pub title: String, pub text: String, pub holdable: bool }` — derives `serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq`
  - `pub fn standard_rules() -> Vec<RuleEntry>` (13 entries, ranks 1–13 in order)
  - `pub fn standard_rules_json() -> String`
  - `pub fn parse_rules(json: &str) -> Vec<RuleEntry>` (panics on corrupt JSON — same trust model as `parse_deck`)
  - `pub fn rule_for_rank(rules: &[RuleEntry], rank: u8) -> &RuleEntry`

- [ ] **Step 1: Add the dependency**

In `drinkinggame/Cargo.toml`, under `serde = { version = "1", features = ["derive"] }`, add:

```toml
serde_json = "1"
```

- [ ] **Step 2: Write the failing tests**

Create `drinkinggame/src/rules.rs` with only the test module:

```rust
//! Ring of Fire rule sets: 13 entries (Ace..King) serialized as JSON in
//! rule_presets.rules_json and snapshotted into games.rules_json at start.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_standard_rules_shape() {
        let rules = standard_rules();
        assert_eq!(rules.len(), 13);
        // Ranks 1..=13 in order.
        for (i, r) in rules.iter().enumerate() {
            assert_eq!(r.rank, i as u8 + 1);
            assert!(!r.title.is_empty());
            assert!(!r.text.is_empty());
        }
        // The group's modifications and holdables.
        assert_eq!(rule_for_rank(&rules, 4).title, "Whores");
        assert_eq!(rule_for_rank(&rules, 6).title, "Dicks");
        assert!(rule_for_rank(&rules, 5).holdable); // Thumb Master
        assert!(rule_for_rank(&rules, 7).holdable); // Heaven
        assert_eq!(rules.iter().filter(|r| r.holdable).count(), 2);
    }

    #[test]
    fn test_rules_json_roundtrip() {
        let rules = standard_rules();
        let json = standard_rules_json();
        assert_eq!(parse_rules(&json), rules);
    }
}
```

Add `pub mod rules;` to `drinkinggame/src/lib.rs`.

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p drinkinggame rules -- --nocapture`
Expected: COMPILE ERROR — `standard_rules` not found.

- [ ] **Step 4: Write the implementation**

Add above the test module in `drinkinggame/src/rules.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RuleEntry {
    pub rank: u8,
    pub title: String,
    pub text: String,
    pub holdable: bool,
}

fn entry(rank: u8, title: &str, text: &str, holdable: bool) -> RuleEntry {
    RuleEntry {
        rank,
        title: title.to_string(),
        text: text.to_string(),
        holdable,
    }
}

/// The group's standard rules, seeded as the "Standard" preset.
pub fn standard_rules() -> Vec<RuleEntry> {
    vec![
        entry(1, "Waterfall", "Everyone drinks; you may only stop when the person before you stops.", false),
        entry(2, "You", "Pick someone to drink.", false),
        entry(3, "Me", "You drink.", false),
        entry(4, "Whores", "Girls drink.", false),
        entry(5, "Thumb Master", "Hold this card. Whenever you put your thumb on the table, last to follow drinks. Spent when used.", true),
        entry(6, "Dicks", "Boys drink.", false),
        entry(7, "Heaven", "Hold this card. Whenever you point up, last to follow drinks. Spent when used.", true),
        entry(8, "Mate", "Pick a mate; they drink whenever you drink.", false),
        entry(9, "Rhyme", "Say a word; go around rhyming with it. First to fail drinks.", false),
        entry(10, "Categories", "Pick a category; go around naming things in it. First to fail drinks.", false),
        entry(11, "Make a Rule", "Invent a rule for the rest of the game. Rule-breakers drink.", false),
        entry(12, "Questions", "Ask anyone a question; they must answer with a question. First to fail drinks.", false),
        entry(13, "King's Cup", "Pour some of your drink into the King's Cup.", false),
    ]
}

pub fn standard_rules_json() -> String {
    serde_json::to_string(&standard_rules()).expect("standard rules serialize")
}

/// Panics on malformed input — rules_json only ever comes from our own
/// serialization, so corruption is a bug, not input.
pub fn parse_rules(json: &str) -> Vec<RuleEntry> {
    serde_json::from_str(json).expect("corrupt rules_json in db")
}

pub fn rule_for_rank(rules: &[RuleEntry], rank: u8) -> &RuleEntry {
    rules
        .iter()
        .find(|r| r.rank == rank)
        .expect("rules_json missing a rank")
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p drinkinggame rules -- --nocapture`
Expected: 2 passed.

- [ ] **Step 6: Lint, format, commit**

```bash
cargo clippy -p drinkinggame && cargo fmt
git add drinkinggame/src/rules.rs drinkinggame/src/lib.rs drinkinggame/Cargo.toml Cargo.lock
git commit -m "feat(drinks): rule-set module with seeded Standard rules"
```

---

### Task 3: Migration 002, preset models, preset CRUD

**Files:**
- Create: `drinkinggame/migrations/002_ring_of_fire.sql`
- Modify: `drinkinggame/src/models.rs` (add `RulePreset`)
- Modify: `drinkinggame/src/db.rs` (run migration 002 + seed; preset CRUD; tests)

**Interfaces:**
- Consumes: `crate::rules::standard_rules_json()` (Task 2).
- Produces:
  - `pub struct RulePreset { pub id: i64, pub name: String, pub rules_json: String, pub created_at: String }` (derives `sqlx::FromRow, Clone, Debug`)
  - `pub async fn list_presets(pool: &DbPool) -> Vec<RulePreset>` (ordered by id — Standard first)
  - `pub async fn get_preset(pool: &DbPool, id: i64) -> Option<RulePreset>`
  - `pub async fn insert_preset(pool: &DbPool, name: &str, rules_json: &str) -> Result<i64, sqlx::Error>` (Err on UNIQUE name violation)
  - `pub async fn update_preset(pool: &DbPool, id: i64, name: &str, rules_json: &str) -> Result<bool, sqlx::Error>` (Ok(false) = no such id; Err = name collision)
  - `pub async fn delete_preset(pool: &DbPool, id: i64) -> bool`

- [ ] **Step 1: Write the migration**

Create `drinkinggame/migrations/002_ring_of_fire.sql`:

```sql
-- Ring of Fire. All timestamps ISO8601 TEXT (portfolio convention).

CREATE TABLE IF NOT EXISTS rule_presets (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    rules_json TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS games (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    room_id INTEGER NOT NULL REFERENCES rooms(id),
    -- Snapshot copied from the preset at start; editing a preset never
    -- mutates a running game.
    rules_json TEXT NOT NULL,
    -- The full shuffled deck as text ("AS,2H,..."): ~150 bytes beats an RNG
    -- seed, whose replay would couple correctness to the RNG never changing.
    deck_order TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    ended_at TEXT
);

-- One active game per room.
CREATE UNIQUE INDEX IF NOT EXISTS idx_games_one_active
    ON games(room_id) WHERE ended_at IS NULL;

CREATE TABLE IF NOT EXISTS game_draws (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    game_id INTEGER NOT NULL REFERENCES games(id),
    player_id INTEGER NOT NULL REFERENCES players(id),
    card_index INTEGER NOT NULL,
    drawn_at TEXT NOT NULL DEFAULT (datetime('now')),
    -- Tombstone for held cards, mirroring events.undone_at.
    spent_at TEXT,
    -- Double-tap race: the loser gets a constraint conflict, not a dupe.
    UNIQUE (game_id, card_index)
);

CREATE INDEX IF NOT EXISTS idx_game_draws_game ON game_draws(game_id);
```

- [ ] **Step 2: Wire migration + seed into run_migrations**

In `drinkinggame/src/db.rs`, replace the body of `run_migrations` with:

```rust
pub async fn run_migrations(pool: &DbPool) {
    sqlx::query(include_str!("../migrations/001_initial.sql"))
        .execute(pool)
        .await
        .expect("drinks migration 001 failed");
    sqlx::query(include_str!("../migrations/002_ring_of_fire.sql"))
        .execute(pool)
        .await
        .expect("drinks migration 002 failed");
    // Seed guard: recreate the Standard preset only if missing, so deleting
    // it is permitted but it returns on next deploy (accepted v1 quirk).
    sqlx::query("INSERT OR IGNORE INTO rule_presets (name, rules_json) VALUES ('Standard', ?1)")
        .bind(crate::rules::standard_rules_json())
        .execute(pool)
        .await
        .expect("standard preset seed failed");
}
```

NOTE: `include_str!` concatenates fine, but sqlx's `query()` only executes a single statement per call on some drivers — SQLite via sqlx DOES support multiple statements in one `execute` for DDL scripts, and migration 001 already relies on this. Keep the same pattern.

Add to `drinkinggame/src/models.rs`:

```rust
#[derive(sqlx::FromRow, Clone, Debug)]
pub struct RulePreset {
    pub id: i64,
    pub name: String,
    pub rules_json: String,
    pub created_at: String,
}
```

- [ ] **Step 3: Write the failing db tests**

Append inside `mod tests` in `drinkinggame/src/db.rs`:

```rust
#[tokio::test]
async fn test_standard_preset_is_seeded_and_seed_is_idempotent() {
    let pool = test_pool().await;
    run_migrations(&pool).await; // second run must not duplicate the seed
    let presets = list_presets(&pool).await;
    assert_eq!(presets.len(), 1);
    assert_eq!(presets[0].name, "Standard");
    let rules = crate::rules::parse_rules(&presets[0].rules_json);
    assert_eq!(rules, crate::rules::standard_rules());
}

#[tokio::test]
async fn test_preset_crud_roundtrip() {
    let pool = test_pool().await;
    let json = crate::rules::standard_rules_json();
    let id = insert_preset(&pool, "House", &json).await.unwrap();
    assert_eq!(get_preset(&pool, id).await.unwrap().name, "House");
    // Duplicate name rejected.
    assert!(insert_preset(&pool, "House", &json).await.is_err());
    // Update name + rules.
    let mut rules = crate::rules::standard_rules();
    rules[3].title = "Floor".to_string();
    let new_json = serde_json::to_string(&rules).unwrap();
    assert!(update_preset(&pool, id, "House 2", &new_json).await.unwrap());
    let got = get_preset(&pool, id).await.unwrap();
    assert_eq!(got.name, "House 2");
    assert_eq!(crate::rules::parse_rules(&got.rules_json)[3].title, "Floor");
    // Update of a missing id reports false.
    assert!(!update_preset(&pool, 9999, "X", &new_json).await.unwrap());
    // Delete.
    assert!(delete_preset(&pool, id).await);
    assert!(get_preset(&pool, id).await.is_none());
    assert!(!delete_preset(&pool, id).await);
}

#[tokio::test]
async fn test_delete_standard_preset_returns_after_migration_rerun() {
    let pool = test_pool().await;
    let standard = &list_presets(&pool).await[0];
    assert!(delete_preset(&pool, standard.id).await);
    assert!(list_presets(&pool).await.is_empty());
    run_migrations(&pool).await; // deploy re-runs migrations
    assert_eq!(list_presets(&pool).await[0].name, "Standard");
}
```

- [ ] **Step 4: Run tests to verify they fail**

Run: `cargo test -p drinkinggame preset -- --nocapture`
Expected: COMPILE ERROR — `list_presets` etc. not found.

- [ ] **Step 5: Implement preset CRUD**

Add to `drinkinggame/src/db.rs` (import `RulePreset` in the existing `use crate::models::…` line):

```rust
pub async fn list_presets(pool: &DbPool) -> Vec<RulePreset> {
    sqlx::query_as::<_, RulePreset>(
        "SELECT id, name, rules_json, created_at FROM rule_presets ORDER BY id",
    )
    .fetch_all(pool)
    .await
    .expect("list_presets failed")
}

pub async fn get_preset(pool: &DbPool, id: i64) -> Option<RulePreset> {
    sqlx::query_as::<_, RulePreset>(
        "SELECT id, name, rules_json, created_at FROM rule_presets WHERE id = ?1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .expect("get_preset failed")
}

/// Returns Err on UNIQUE violation (name taken) — callers map it to a
/// friendly error.
pub async fn insert_preset(
    pool: &DbPool,
    name: &str,
    rules_json: &str,
) -> Result<i64, sqlx::Error> {
    let res = sqlx::query("INSERT INTO rule_presets (name, rules_json) VALUES (?1, ?2)")
        .bind(name)
        .bind(rules_json)
        .execute(pool)
        .await?;
    Ok(res.last_insert_rowid())
}

/// Ok(false) when the id doesn't exist; Err on a name collision.
pub async fn update_preset(
    pool: &DbPool,
    id: i64,
    name: &str,
    rules_json: &str,
) -> Result<bool, sqlx::Error> {
    let res = sqlx::query("UPDATE rule_presets SET name = ?2, rules_json = ?3 WHERE id = ?1")
        .bind(id)
        .bind(name)
        .bind(rules_json)
        .execute(pool)
        .await?;
    Ok(res.rows_affected() > 0)
}

pub async fn delete_preset(pool: &DbPool, id: i64) -> bool {
    let res = sqlx::query("DELETE FROM rule_presets WHERE id = ?1")
        .bind(id)
        .execute(pool)
        .await
        .expect("delete_preset failed");
    res.rows_affected() > 0
}
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test -p drinkinggame -- --nocapture`
Expected: all pass, including the 3 new preset tests AND every pre-existing test (migration change must not break them).

- [ ] **Step 7: Lint, format, commit**

```bash
cargo clippy -p drinkinggame && cargo fmt
git add drinkinggame/migrations/002_ring_of_fire.sql drinkinggame/src/models.rs drinkinggame/src/db.rs
git commit -m "feat(drinks): ring of fire schema and rule-preset CRUD"
```

---

### Task 4: Game lifecycle db functions + error variants

**Files:**
- Modify: `drinkinggame/src/error.rs` (5 new variants)
- Modify: `drinkinggame/src/models.rs` (add `Game`, `DrawRow`, `DrawCount`)
- Modify: `drinkinggame/src/db.rs` (game queries + tests)

**Interfaces:**
- Consumes: migration 002 tables (Task 3).
- Produces:
  - `GameError` gains: `NoActiveGame`, `GameAlreadyActive`, `DeckExhausted`, `CardNotHeld`, `PresetNotFound`
  - `pub struct Game { pub id: i64, pub room_id: i64, pub rules_json: String, pub deck_order: String, pub created_at: String, pub ended_at: Option<String> }`
  - `pub struct DrawRow { pub id: i64, pub player_id: i64, pub player_name: String, pub card_index: i64, pub spent_at: Option<String> }`
  - `pub struct DrawCount { pub name: String, pub draws: i64 }` (derives `PartialEq` too)
  - `pub async fn start_game(pool, room_id: i64, rules_json: &str, deck_order: &str) -> Result<i64, GameError>` (`GameAlreadyActive` on the partial-unique-index violation)
  - `pub async fn get_active_game(pool, room_id: i64) -> Option<Game>`
  - `pub async fn insert_draw(pool, game_id: i64, player_id: i64) -> Result<i64, GameError>` — returns the claimed `card_index`; `DeckExhausted` at 52
  - `pub async fn get_draws(pool, game_id: i64) -> Vec<DrawRow>` (ordered by card_index)
  - `pub async fn spend_draw(pool, game_id: i64, draw_id: i64, player_id: i64) -> bool` (false unless the row exists **in that game**, belongs to player, and is unspent)
  - `pub async fn end_game(pool, game_id: i64)`
  - `pub async fn draw_counts(pool, game_id: i64) -> Vec<DrawCount>` (desc by draws, then name)

- [ ] **Step 1: Add error variants**

In `drinkinggame/src/error.rs`, extend the enum (before the `Db` variant):

```rust
    #[error("no Ring of Fire game is running")]
    NoActiveGame,
    #[error("a game is already running in this room")]
    GameAlreadyActive,
    #[error("the deck is empty")]
    DeckExhausted,
    #[error("you don't hold that card")]
    CardNotHeld,
    #[error("no preset with that id")]
    PresetNotFound,
```

And extend the status match in `into_response`:

```rust
            GameError::NoActiveGame | GameError::PresetNotFound => StatusCode::NOT_FOUND,
            GameError::GameAlreadyActive | GameError::DeckExhausted => StatusCode::CONFLICT,
            GameError::CardNotHeld => StatusCode::FORBIDDEN,
```

- [ ] **Step 2: Add models**

Append to `drinkinggame/src/models.rs`:

```rust
#[derive(sqlx::FromRow, Clone, Debug)]
pub struct Game {
    pub id: i64,
    pub room_id: i64,
    pub rules_json: String,
    pub deck_order: String,
    pub created_at: String,
    pub ended_at: Option<String>,
}

/// A draw joined with the drawer's name, for rendering.
#[derive(sqlx::FromRow, Clone, Debug)]
pub struct DrawRow {
    pub id: i64,
    pub player_id: i64,
    pub player_name: String,
    pub card_index: i64,
    pub spent_at: Option<String>,
}

#[derive(sqlx::FromRow, Clone, Debug, PartialEq)]
pub struct DrawCount {
    pub name: String,
    pub draws: i64,
}
```

- [ ] **Step 3: Write the failing db tests**

Append inside `mod tests` in `drinkinggame/src/db.rs`:

```rust
async fn seed_game(pool: &DbPool) -> (i64, i64, i64, i64) {
    let (room, alice, bob) = seed_room_with_players(pool).await;
    let deck = crate::cards::deck_to_string(&crate::cards::shuffled_deck());
    let game = start_game(pool, room, &crate::rules::standard_rules_json(), &deck)
        .await
        .unwrap();
    (room, game, alice, bob)
}

#[tokio::test]
async fn test_one_active_game_per_room() {
    let pool = test_pool().await;
    let (room, _game, _a, _b) = seed_game(&pool).await;
    let deck = crate::cards::deck_to_string(&crate::cards::shuffled_deck());
    let err = start_game(&pool, room, "[]", &deck).await.unwrap_err();
    assert!(matches!(err, crate::error::GameError::GameAlreadyActive));
    // Ending frees the room for a new game.
    let game = get_active_game(&pool, room).await.unwrap();
    end_game(&pool, game.id).await;
    assert!(get_active_game(&pool, room).await.is_none());
    assert!(start_game(&pool, room, "[]", &deck).await.is_ok());
}

#[tokio::test]
async fn test_draws_come_back_in_deck_order() {
    let pool = test_pool().await;
    let (_room, game, alice, bob) = seed_game(&pool).await;
    assert_eq!(insert_draw(&pool, game, alice).await.unwrap(), 0);
    assert_eq!(insert_draw(&pool, game, bob).await.unwrap(), 1);
    assert_eq!(insert_draw(&pool, game, alice).await.unwrap(), 2);
    let draws = get_draws(&pool, game).await;
    assert_eq!(
        draws.iter().map(|d| d.card_index).collect::<Vec<_>>(),
        vec![0, 1, 2]
    );
    assert_eq!(draws[0].player_name, "alice");
    assert_eq!(draws[1].player_name, "bob");
}

#[tokio::test]
async fn test_double_draw_on_same_index_conflicts_and_retries() {
    let pool = test_pool().await;
    let (_room, game, alice, bob) = seed_game(&pool).await;
    // Simulate alice's in-flight draw landing first on index 0.
    sqlx::query("INSERT INTO game_draws (game_id, player_id, card_index) VALUES (?1, ?2, 0)")
        .bind(game)
        .bind(alice)
        .execute(&pool)
        .await
        .unwrap();
    // Bob's insert_draw must skip to index 1, not fail or duplicate.
    assert_eq!(insert_draw(&pool, game, bob).await.unwrap(), 1);
}

#[tokio::test]
async fn test_deck_exhaustion() {
    let pool = test_pool().await;
    let (_room, game, alice, _bob) = seed_game(&pool).await;
    for i in 0..52 {
        assert_eq!(insert_draw(&pool, game, alice).await.unwrap(), i);
    }
    let err = insert_draw(&pool, game, alice).await.unwrap_err();
    assert!(matches!(err, crate::error::GameError::DeckExhausted));
}

#[tokio::test]
async fn test_spend_only_holder_only_once() {
    let pool = test_pool().await;
    let (_room, game, alice, bob) = seed_game(&pool).await;
    insert_draw(&pool, game, alice).await.unwrap();
    let draw_id = get_draws(&pool, game).await[0].id;

    assert!(!spend_draw(&pool, game, draw_id, bob).await); // not the holder
    assert!(!spend_draw(&pool, game + 1, draw_id, alice).await); // wrong game
    assert!(spend_draw(&pool, game, draw_id, alice).await); // holder spends
    assert!(!spend_draw(&pool, game, draw_id, alice).await); // already spent
    assert!(!spend_draw(&pool, game, 9999, alice).await); // no such draw
    assert!(get_draws(&pool, game).await[0].spent_at.is_some());
}

#[tokio::test]
async fn test_draw_counts_order_and_totals() {
    let pool = test_pool().await;
    let (_room, game, alice, bob) = seed_game(&pool).await;
    insert_draw(&pool, game, bob).await.unwrap();
    insert_draw(&pool, game, alice).await.unwrap();
    insert_draw(&pool, game, bob).await.unwrap();
    assert_eq!(
        draw_counts(&pool, game).await,
        vec![
            DrawCount { name: "bob".into(), draws: 2 },
            DrawCount { name: "alice".into(), draws: 1 },
        ]
    );
}
```

Also add `DrawCount` to the test module's imports if not covered by `use super::*;` (it is, via the `use crate::models::…` addition in Step 4).

- [ ] **Step 4: Run tests to verify they fail**

Run: `cargo test -p drinkinggame -- --nocapture 2>&1 | head -30`
Expected: COMPILE ERROR — `start_game` etc. not found.

- [ ] **Step 5: Implement game queries**

In `drinkinggame/src/db.rs`, extend the models import to `use crate::models::{DrawCount, DrawRow, Game, LeaderboardRow, Player, Room, RulePreset};`, add `use crate::error::GameError;`, then append:

```rust
/// GameAlreadyActive when the partial unique index (one active game per
/// room) rejects the insert.
pub async fn start_game(
    pool: &DbPool,
    room_id: i64,
    rules_json: &str,
    deck_order: &str,
) -> Result<i64, GameError> {
    let res = sqlx::query("INSERT INTO games (room_id, rules_json, deck_order) VALUES (?1, ?2, ?3)")
        .bind(room_id)
        .bind(rules_json)
        .bind(deck_order)
        .execute(pool)
        .await;
    match res {
        Ok(r) => Ok(r.last_insert_rowid()),
        Err(e) if e.as_database_error().is_some_and(|d| d.is_unique_violation()) => {
            Err(GameError::GameAlreadyActive)
        }
        Err(e) => Err(e.into()),
    }
}

pub async fn get_active_game(pool: &DbPool, room_id: i64) -> Option<Game> {
    sqlx::query_as::<_, Game>(
        "SELECT id, room_id, rules_json, deck_order, created_at, ended_at
         FROM games WHERE room_id = ?1 AND ended_at IS NULL",
    )
    .bind(room_id)
    .fetch_optional(pool)
    .await
    .expect("get_active_game failed")
}

/// Claims the next undrawn card index for player_id and returns it.
/// A double-tap race loses on UNIQUE(game_id, card_index) and retries with
/// the next index. Terminates: at most 52 conflicts before DeckExhausted.
pub async fn insert_draw(pool: &DbPool, game_id: i64, player_id: i64) -> Result<i64, GameError> {
    loop {
        let (count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM game_draws WHERE game_id = ?1")
                .bind(game_id)
                .fetch_one(pool)
                .await
                .map_err(GameError::from)?;
        if count >= 52 {
            return Err(GameError::DeckExhausted);
        }
        let res =
            sqlx::query("INSERT INTO game_draws (game_id, player_id, card_index) VALUES (?1, ?2, ?3)")
                .bind(game_id)
                .bind(player_id)
                .bind(count)
                .execute(pool)
                .await;
        match res {
            Ok(_) => return Ok(count),
            Err(e) if e.as_database_error().is_some_and(|d| d.is_unique_violation()) => continue,
            Err(e) => return Err(e.into()),
        }
    }
}

pub async fn get_draws(pool: &DbPool, game_id: i64) -> Vec<DrawRow> {
    sqlx::query_as::<_, DrawRow>(
        "SELECT gd.id, gd.player_id, p.name AS player_name, gd.card_index, gd.spent_at
         FROM game_draws gd JOIN players p ON p.id = gd.player_id
         WHERE gd.game_id = ?1 ORDER BY gd.card_index",
    )
    .bind(game_id)
    .fetch_all(pool)
    .await
    .expect("get_draws failed")
}

/// True only when the draw exists in game_id, belongs to player_id, and is
/// unspent — the game_id guard stops spends against draws from ended games.
pub async fn spend_draw(pool: &DbPool, game_id: i64, draw_id: i64, player_id: i64) -> bool {
    let res = sqlx::query(
        "UPDATE game_draws SET spent_at = datetime('now')
         WHERE id = ?1 AND player_id = ?2 AND game_id = ?3 AND spent_at IS NULL",
    )
    .bind(draw_id)
    .bind(player_id)
    .bind(game_id)
    .execute(pool)
    .await
    .expect("spend_draw failed");
    res.rows_affected() > 0
}

pub async fn end_game(pool: &DbPool, game_id: i64) {
    sqlx::query("UPDATE games SET ended_at = datetime('now') WHERE id = ?1 AND ended_at IS NULL")
        .bind(game_id)
        .execute(pool)
        .await
        .expect("end_game failed");
}

/// Per-player draw totals, most draws first, then name for stable order.
pub async fn draw_counts(pool: &DbPool, game_id: i64) -> Vec<DrawCount> {
    sqlx::query_as::<_, DrawCount>(
        "SELECT p.name, COUNT(*) AS draws
         FROM game_draws gd JOIN players p ON p.id = gd.player_id
         WHERE gd.game_id = ?1
         GROUP BY p.id ORDER BY draws DESC, p.name ASC",
    )
    .bind(game_id)
    .fetch_all(pool)
    .await
    .expect("draw_counts failed")
}
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test -p drinkinggame -- --nocapture 2>&1 | tail -15`
Expected: all pass (6 new game tests + everything prior).

- [ ] **Step 7: Lint, format, commit**

```bash
cargo clippy -p drinkinggame && cargo fmt
git add drinkinggame/src/error.rs drinkinggame/src/models.rs drinkinggame/src/db.rs
git commit -m "feat(drinks): game lifecycle queries with race-safe draws"
```

---

### Task 5: Hub variant + game panel rendering

**Files:**
- Modify: `drinkinggame/src/hub.rs` (add `RoomMessage::Game(String)`)
- Modify: `drinkinggame/src/render.rs` (card face + panel builders + tests)

**Interfaces:**
- Consumes: `cards::Card`/`Suit` (Task 1), `rules::RuleEntry` (Task 2), `models::{DrawCount, RulePreset}` (Tasks 3–4).
- Produces:
  - `RoomMessage::Game(String)` — rendered game panel HTML, swapped into `#game-panel` on every screen
  - `pub struct CurrentCard { pub card: Card, pub title: String, pub text: String, pub drawer: String }`
  - `pub struct HeldCardView { pub draw_id: i64, pub holder_id: i64, pub holder_name: String, pub card: Card, pub title: String }`
  - `pub struct GameView<'a> { pub base_path: &'a str, pub code: &'a str, pub current: Option<CurrentCard>, pub remaining: i64, pub held: Vec<HeldCardView>, pub counts: &'a [DrawCount], pub announcement: Option<String> }`
  - `pub fn card_face_html(card: Card) -> String`
  - `pub fn game_idle_panel(base_path: &str, code: &str, presets: &[RulePreset]) -> String`
  - `pub fn game_active_panel(view: &GameView) -> String`
  - `pub fn game_summary_panel(counts: &[DrawCount]) -> String`

- [ ] **Step 1: Add the hub variant**

In `drinkinggame/src/hub.rs`, extend the enum:

```rust
    /// Rendered Ring of Fire panel HTML for the #game-panel container.
    Game(String),
```

- [ ] **Step 2: Write the failing render tests**

Append inside `mod tests` in `drinkinggame/src/render.rs`:

```rust
use crate::cards::{Card, Suit};
use crate::models::{DrawCount, RulePreset};

fn preset(id: i64, name: &str) -> RulePreset {
    RulePreset {
        id,
        name: name.into(),
        rules_json: crate::rules::standard_rules_json(),
        created_at: "2026-07-29".into(),
    }
}

#[test]
fn test_card_face_marks_red_suits() {
    let html = card_face_html(Card { rank: 12, suit: Suit::Hearts });
    assert!(html.contains("Q"));
    assert!(html.contains("\u{2665}"));
    assert!(html.contains("card-red"));
    let html = card_face_html(Card { rank: 1, suit: Suit::Spades });
    assert!(!html.contains("card-red"));
}

#[test]
fn test_idle_panel_lists_presets_and_start() {
    let html = game_idle_panel("/drinks", "ABCD", &[preset(1, "Standard"), preset(2, "<Wild>")]);
    assert!(html.contains("/drinks/room/ABCD/game/start"));
    assert!(html.contains(r#"<option value="1">Standard</option>"#));
    assert!(html.contains("&lt;Wild&gt;")); // names escaped
    assert!(html.contains("Start Ring of Fire"));
}

#[test]
fn test_active_panel_shows_card_held_and_counts() {
    let counts = vec![DrawCount { name: "alice".into(), draws: 3 }];
    let view = GameView {
        base_path: "/drinks",
        code: "ABCD",
        current: Some(CurrentCard {
            card: Card { rank: 5, suit: Suit::Clubs },
            title: "Thumb Master".into(),
            text: "Thumbs!".into(),
            drawer: "alice".into(),
        }),
        remaining: 49,
        held: vec![HeldCardView {
            draw_id: 7,
            holder_id: 2,
            holder_name: "bob".into(),
            card: Card { rank: 7, suit: Suit::Hearts },
            title: "Heaven".into(),
        }],
        counts: &counts,
        announcement: Some("bob used Heaven!".into()),
    };
    let html = game_active_panel(&view);
    assert!(html.contains("Thumb Master"));
    assert!(html.contains("alice drew"));
    assert!(html.contains("49 cards left"));
    assert!(html.contains("/drinks/room/ABCD/game/draw"));
    assert!(html.contains("/drinks/room/ABCD/game/end"));
    // Use button: hidden by default, tagged with holder + draw ids so the
    // page JS reveals it only on the holder's phone.
    assert!(html.contains(r#"data-holder-id="2""#));
    assert!(html.contains(r#"data-draw-id="7""#));
    assert!(html.contains("hidden"));
    assert!(html.contains("bob used Heaven!"));
    assert!(html.contains("alice") && html.contains("3"));
}

#[test]
fn test_active_panel_before_first_draw_has_no_current_card() {
    let counts: Vec<DrawCount> = vec![];
    let view = GameView {
        base_path: "",
        code: "ABCD",
        current: None,
        remaining: 52,
        held: vec![],
        counts: &counts,
        announcement: None,
    };
    let html = game_active_panel(&view);
    assert!(html.contains("52 cards left"));
    assert!(html.contains("Tap to draw"));
}

#[test]
fn test_summary_panel() {
    let counts = vec![
        DrawCount { name: "alice".into(), draws: 30 },
        DrawCount { name: "<bob>".into(), draws: 22 },
    ];
    let html = game_summary_panel(&counts);
    assert!(html.contains("Game over"));
    assert!(html.contains("alice"));
    assert!(html.contains("30"));
    assert!(html.contains("&lt;bob&gt;"));
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p drinkinggame render -- --nocapture 2>&1 | head -20`
Expected: COMPILE ERROR — `card_face_html` etc. not found.

- [ ] **Step 4: Implement the fragment builders**

Add to `drinkinggame/src/render.rs` (top-level imports: `use crate::cards::Card; use crate::models::{DrawCount, RulePreset};`):

```rust
pub struct CurrentCard {
    pub card: Card,
    pub title: String,
    pub text: String,
    pub drawer: String,
}

pub struct HeldCardView {
    pub draw_id: i64,
    pub holder_id: i64,
    pub holder_name: String,
    pub card: Card,
    pub title: String,
}

pub struct GameView<'a> {
    pub base_path: &'a str,
    pub code: &'a str,
    pub current: Option<CurrentCard>,
    pub remaining: i64,
    pub held: Vec<HeldCardView>,
    pub counts: &'a [DrawCount],
    pub announcement: Option<String>,
}

/// A card face in pure HTML/CSS — rank + suit glyph, red/black via class.
pub fn card_face_html(card: Card) -> String {
    let red = if card.suit.is_red() { " card-red" } else { "" };
    format!(
        r#"<div class="card-face{red}"><span class="card-rank">{}</span><span class="card-suit">{}</span></div>"#,
        card.rank_label(),
        card.suit.glyph(),
    )
}

/// Idle state: preset picker + start button. First preset (Standard, lowest
/// id) is the <select> default by position.
pub fn game_idle_panel(base_path: &str, code: &str, presets: &[RulePreset]) -> String {
    let options: String = presets
        .iter()
        .map(|p| format!(r#"<option value="{}">{}</option>"#, p.id, html_escape(&p.name)))
        .collect();
    format!(
        r#"<div class="game-idle">
<form hx-post="{base_path}/room/{code}/game/start" hx-swap="none">
<select name="preset_id">{options}</select>
<button type="submit" class="btn-start">Start Ring of Fire</button>
</form>
<a class="presets-link" href="{base_path}/presets">Edit rule presets</a>
</div>"#
    )
}

fn draw_counts_html(counts: &[DrawCount]) -> String {
    counts
        .iter()
        .map(|c| {
            format!(
                r#"<li><span class="dc-name">{}</span><span class="dc-count">{}</span></li>"#,
                html_escape(&c.name),
                c.draws
            )
        })
        .collect()
}

/// The live game panel: announcement, current card + rule, deck button,
/// held-card strip, per-player draw counts, end-early button.
pub fn game_active_panel(view: &GameView) -> String {
    let base_path = view.base_path;
    let code = view.code;
    let announcement = view
        .announcement
        .as_deref()
        .map(|a| format!(r#"<p class="game-announcement">{}</p>"#, html_escape(a)))
        .unwrap_or_default();
    let current = match &view.current {
        Some(c) => format!
            (r#"<div class="game-current">{}<div class="game-rule"><p class="rule-drawer">{} drew</p><h3 class="rule-title">{}</h3><p class="rule-text">{}</p></div></div>"#,
            card_face_html(c.card),
            html_escape(&c.drawer),
            html_escape(&c.title),
            html_escape(&c.text),
        ),
        None => r#"<div class="game-current"><p class="rule-text">Fresh deck. Tap to draw the first card.</p></div>"#.to_string(),
    };
    let held: String = if view.held.is_empty() {
        String::new()
    } else {
        let items: String = view
            .held
            .iter()
            .map(|h| {
                format!(
                    r#"<li class="held-card">{}<span class="held-holder">{} · {}</span><button class="use-btn" hidden data-holder-id="{}" data-draw-id="{}" hx-post="{base_path}/room/{code}/game/spend" hx-vals='{{"draw_id":{}}}' hx-swap="none">Use</button></li>"#,
                    card_face_html(h.card),
                    html_escape(&h.holder_name),
                    html_escape(&h.title),
                    h.holder_id,
                    h.draw_id,
                    h.draw_id,
                )
            })
            .collect();
        format!(r#"<ul class="held-strip">{items}</ul>"#)
    };
    format!(
        r#"<div class="game-active">
{announcement}
{current}
<button class="btn-draw" hx-post="{base_path}/room/{code}/game/draw" hx-swap="none">Tap to draw<span class="deck-count">{} cards left</span></button>
{held}
<ol class="draw-counts">{}</ol>
<button class="btn-game-end" hx-post="{base_path}/room/{code}/game/end" hx-swap="none" hx-confirm="End the game for everyone?">End game early</button>
</div>"#,
        view.remaining,
        draw_counts_html(view.counts),
    )
}

/// Post-game summary. The idle panel (rendered separately) restores Start.
pub fn game_summary_panel(counts: &[DrawCount]) -> String {
    format!(
        r#"<div class="game-summary"><h3>Game over</h3><ol class="draw-counts">{}</ol></div>"#,
        draw_counts_html(counts),
    )
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p drinkinggame render -- --nocapture`
Expected: 5 new tests pass alongside the 3 existing render tests. Also run `cargo test -p drinkinggame hub` — the existing hub test must still compile (the new variant is additive; its match arms use catch-alls or explicit variants only).

- [ ] **Step 6: Lint, format, commit**

```bash
cargo clippy -p drinkinggame && cargo fmt
git add drinkinggame/src/hub.rs drinkinggame/src/render.rs
git commit -m "feat(drinks): game panel fragments and Game broadcast variant"
```

---

### Task 6: Game routes, page/SSE integration, CSS

**Files:**
- Create: `drinkinggame/src/game.rs`
- Modify: `drinkinggame/src/lib.rs` (add `pub mod game;`)
- Modify: `drinkinggame/src/routes.rs` (register routes; pass panel + player_id to templates; SSE game event)
- Modify: `drinkinggame/templates/room.html`, `drinkinggame/templates/screen.html`
- Modify: `drinkinggame/assets/game.css`
- Test: `drinkinggame/tests/http.rs`

**Interfaces:**
- Consumes: everything from Tasks 1–5 (exact signatures listed in those tasks).
- Produces:
  - `POST /room/{code}/game/start` (form `preset_id: i64`), `POST /room/{code}/game/draw`, `POST /room/{code}/game/spend` (form `draw_id: i64`), `POST /room/{code}/game/end` — all member-only, all `204` on success
  - `pub async fn current_panel(state: &GameState, room_id: i64, code: &str, announcement: Option<String>) -> String` in `game.rs` — used by `routes.rs` for page render and SSE snapshot
  - SSE event name `game` carrying panel HTML for `#game-panel`

- [ ] **Step 1: Write the failing integration tests**

In `drinkinggame/tests/http.rs`, first refactor the pool helper so tests can reach the db directly:

```rust
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
```

Then append the game tests:

```rust
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

    let res = post_form(&app, &cookie, &format!("/room/{code}/game/start"), "preset_id=1").await;
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
    let res = post_form(&app, &cookie, &format!("/room/{code}/game/start"), "preset_id=999").await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    // Start while a game is running.
    post_form(&app, &cookie, &format!("/room/{code}/game/start"), "preset_id=1").await;
    let res = post_form(&app, &cookie, &format!("/room/{code}/game/start"), "preset_id=1").await;
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
    let res = post_form(&app, &bob, &format!("/room/{code}/game/spend"), &format!("draw_id={draw_id}")).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    // Alice spends it; second spend fails.
    let res = post_form(&app, &alice, &format!("/room/{code}/game/spend"), &format!("draw_id={draw_id}")).await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    let res = post_form(&app, &alice, &format!("/room/{code}/game/spend"), &format!("draw_id={draw_id}")).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    // Held strip is gone from the page.
    assert!(!room_page_html(&app, &alice, &code).await.contains("held-strip"));
}

#[tokio::test]
async fn test_52nd_draw_auto_ends_game() {
    let app = test_app().await;
    let cookie = login(&app, "alice", "1234").await;
    let code = create_room(&app, &cookie).await;
    post_form(&app, &cookie, &format!("/room/{code}/game/start"), "preset_id=1").await;
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

#[tokio::test]
async fn test_end_game_early() {
    let app = test_app().await;
    let cookie = login(&app, "alice", "1234").await;
    let code = create_room(&app, &cookie).await;
    post_form(&app, &cookie, &format!("/room/{code}/game/start"), "preset_id=1").await;
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
    post_form(&app, &cookie, &format!("/room/{code}/game/start"), "preset_id=1").await;

    // Spectator page renders the panel server-side.
    let res = app
        .clone()
        .oneshot(Request::get(format!("/room/{code}/screen")).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert!(body_string(res).await.contains("52 cards left"));

    // SSE: initial game snapshot, then a draw pushes a fresh panel.
    let res = app
        .clone()
        .oneshot(Request::get(format!("/room/{code}/sse")).body(Body::empty()).unwrap())
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
```

NOTE: SSE frames put each event on its own chunk in practice, but if a frame arrives split, read further chunks before asserting (only adjust if the test proves flaky — the existing SSE test uses the same one-frame-per-chunk assumption).

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p drinkinggame --test http 2>&1 | head -20`
Expected: COMPILE ERROR — routes and `current_panel` don't exist yet.

- [ ] **Step 3: Implement `drinkinggame/src/game.rs`**

```rust
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
use crate::models::{Game, Player, Room};
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
        Some(game) => active_panel(state, &game, code, announcement).await,
        None => idle_panel(state, code).await,
    }
}

async fn idle_panel(state: &GameState, code: &str) -> String {
    let presets = db::list_presets(&state.pool).await;
    render::game_idle_panel(&state.base_path, code, &presets)
}

async fn active_panel(
    state: &GameState,
    game: &Game,
    code: &str,
    announcement: Option<String>,
) -> String {
    let deck = cards::parse_deck(&game.deck_order);
    let rules = rules::parse_rules(&game.rules_json);
    let draws = db::get_draws(&state.pool, game.id).await;
    let counts = db::draw_counts(&state.pool, game.id).await;

    let current = draws.last().map(|d| {
        let card = deck[d.card_index as usize];
        let rule = rules::rule_for_rank(&rules, card.rank);
        render::CurrentCard {
            card,
            title: rule.title.clone(),
            text: rule.text.clone(),
            drawer: d.player_name.clone(),
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

    let view = render::GameView {
        base_path: &state.base_path,
        code,
        current,
        remaining: 52 - draws.len() as i64,
        held,
        counts: &counts,
        announcement,
    };
    render::game_active_panel(&view)
}

async fn broadcast_panel(state: &GameState, room_id: i64, code: &str, announcement: Option<String>) {
    let html = current_panel(state, room_id, code, announcement).await;
    state.hub.publish(room_id, RoomMessage::Game(html));
}

/// End-of-game broadcast: summary on top, idle panel (Start button) below.
/// Page reloads render just the idle panel — the summary is transient.
async fn broadcast_game_over(state: &GameState, room_id: i64, code: &str, game_id: i64) {
    let counts = db::draw_counts(&state.pool, game_id).await;
    let html = format!(
        "{}{}",
        render::game_summary_panel(&counts),
        idle_panel(state, code).await
    );
    state.hub.publish(room_id, RoomMessage::Game(html));
}

/// Shared guard: open room + membership, mirroring log_event's checks.
async fn member_room(
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
    if let Err(e) = db::start_game(&state.pool, room.id, &preset.rules_json, &deck).await {
        return e.into_response();
    }
    db::touch_room(&state.pool, room.id).await;
    broadcast_panel(&state, room.id, &room.code, None).await;
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
    let index = match db::insert_draw(&state.pool, game.id, player.id).await {
        Ok(i) => i,
        Err(e) => return e.into_response(),
    };
    db::touch_room(&state.pool, room.id).await;
    if index == 51 {
        // Last card: auto-end and broadcast the summary.
        db::end_game(&state.pool, game.id).await;
        broadcast_panel(&state, room.id, &room.code, None).await; // show the final card…
        broadcast_game_over(&state, room.id, &room.code, game.id).await; // …then the summary
    } else {
        broadcast_panel(&state, room.id, &room.code, None).await;
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
        .map(|d| rules::rule_for_rank(&rules, deck[d.card_index as usize].rank).title.clone())
        .unwrap_or_default();
    let announcement = format!("{} used {}!", player.name, title);
    broadcast_panel(&state, room.id, &room.code, Some(announcement)).await;
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
    db::end_game(&state.pool, game.id).await;
    db::touch_room(&state.pool, room.id).await;
    broadcast_game_over(&state, room.id, &room.code, game.id).await;
    StatusCode::NO_CONTENT.into_response()
}
```

Add `pub mod game;` to `drinkinggame/src/lib.rs`.

- [ ] **Step 4: Wire routes, templates, SSE**

In `drinkinggame/src/routes.rs`:

1. Register in `router()` (after the `/room/{code}/end` line):

```rust
        .route("/room/{code}/game/start", post(crate::game::start_game_handler))
        .route("/room/{code}/game/draw", post(crate::game::draw_handler))
        .route("/room/{code}/game/spend", post(crate::game::spend_handler))
        .route("/room/{code}/game/end", post(crate::game::end_game_handler))
```

2. `RoomTemplate` gains two fields: `player_id: i64` and `game_panel: String`. In `room_page`, after the leaderboard fetch:

```rust
    let game_panel = crate::game::current_panel(&state, room.id, &code, None).await;
```

and pass `player_id: player.id` and `game_panel` into the template (note: read `player.id` before `player.name` is moved).

3. `ScreenTemplate` gains `game_panel: String`; `screen_page` builds it the same way.

4. In `sse_stream`, replace the single initial event with a two-event snapshot (leaderboard, then game), and map the new variant. The snapshot must be rendered BEFORE building the stream (after the subscribe + ended re-check):

```rust
    let rows = db::leaderboard(&state.pool, room.id).await;
    let initial = render::leaderboard_items(&rows);
    let initial_game = crate::game::current_panel(&state, room.id, &room.code, None).await;

    let stream = futures::stream::iter([
        Ok::<_, Infallible>(Event::default().event("leaderboard").data(initial)),
        Ok::<_, Infallible>(Event::default().event("game").data(initial_game)),
    ])
    .chain(BroadcastStream::new(rx).filter_map(|msg| async move {
        match msg {
            Ok(RoomMessage::Leaderboard(html)) => {
                Some(Ok(Event::default().event("leaderboard").data(html)))
            }
            Ok(RoomMessage::Game(html)) => Some(Ok(Event::default().event("game").data(html))),
            Ok(RoomMessage::Ended) => Some(Ok(Event::default().event("ended").data(""))),
            // Lagged receiver: skip — the next update carries full state anyway.
            Err(_) => None,
        }
    }));
```

CAUTION: SSE `data:` frames cannot contain raw newlines as a single line — axum's `Event::data` handles multi-line strings by emitting multiple `data:` lines, and `EventSource` rejoins them with `\n`. The panel HTML contains newlines; this works, no action needed. (The existing leaderboard fragments are single-line, so this is the first multi-line payload — worth knowing, not fixing.)

5. In `drinkinggame/templates/room.html`:
- Change `<body>` to `<body data-player-id="{{ player_id }}">`
- After the `.btn-row` divs, before the leaderboard, insert:

```html
  <div id="game-error"></div>
  <div id="game-panel">{{ game_panel|safe }}</div>
```

- Replace the `<script>` block with:

```html
  <script>
    const es = new EventSource("{{ base_path }}/room/{{ code }}/sse");
    es.addEventListener("leaderboard", (e) => {
      document.getElementById("leaderboard").innerHTML = e.data;
    });
    es.addEventListener("game", (e) => {
      const panel = document.getElementById("game-panel");
      panel.innerHTML = e.data;
      htmx.process(panel); // SSE-injected HTML bypasses htmx's own scan
      revealUseButtons();
    });
    es.addEventListener("ended", () => {
      es.close();
      window.location = "{{ base_path }}/";
    });
    // Use buttons arrive hidden in the broadcast HTML (same fragment for
    // every screen); reveal only the ones this player holds.
    function revealUseButtons() {
      const me = document.body.dataset.playerId;
      document.querySelectorAll("#game-panel .use-btn").forEach((b) => {
        if (b.dataset.holderId === me) b.hidden = false;
      });
    }
    document.addEventListener("DOMContentLoaded", revealUseButtons);
    // Surface 4xx game errors (already-friendly HTML fragments) briefly.
    document.body.addEventListener("htmx:responseError", (e) => {
      const el = document.getElementById("game-error");
      if (!el) return;
      el.innerHTML = e.detail.xhr.responseText;
      setTimeout(() => { el.innerHTML = ""; }, 4000);
    });
  </script>
```

6. In `drinkinggame/templates/screen.html`, after the room-code line insert `<div id="game-panel">{{ game_panel|safe }}</div>`, and add a game listener to its script (no htmx on the screen page — the panel is view-only there, buttons are hidden by CSS):

```js
    es.addEventListener("game", (e) => {
      document.getElementById("game-panel").innerHTML = e.data;
    });
```

- [ ] **Step 5: Add CSS**

Append to `drinkinggame/assets/game.css` under a section comment:

```css
/* --- Ring of Fire --- */
#game-panel { width: 100%; max-width: 420px; }
#game-error { width: 100%; max-width: 420px; }
.game-idle form { display: flex; gap: 0.5rem; }
.game-idle select { flex: 1; }
.presets-link { font-size: 0.9rem; color: var(--muted); }
.card-face {
  display: inline-flex; flex-direction: column; align-items: center;
  justify-content: center; width: 4.2rem; height: 6rem; flex-shrink: 0;
  border: 2px solid #222; border-radius: 0.5rem;
  background: #fff; color: #111; font-weight: 800;
}
.card-rank { font-size: 1.5rem; line-height: 1; }
.card-suit { font-size: 1.3rem; line-height: 1.2; }
.card-red { color: #c0392b; }
.game-current { display: flex; gap: 0.9rem; align-items: center; margin: 0.75rem 0; }
.rule-drawer { color: var(--muted); font-size: 0.85rem; margin: 0; }
.rule-title { margin: 0.1rem 0; }
.rule-text { margin: 0; font-size: 0.95rem; }
.game-announcement { font-weight: 700; }
.btn-draw { width: 100%; min-height: 4.5rem; display: flex; flex-direction: column; align-items: center; justify-content: center; }
.deck-count { color: var(--muted); font-size: 0.85rem; font-weight: 400; }
.held-strip { display: flex; gap: 0.75rem; list-style: none; padding: 0; overflow-x: auto; }
.held-card { display: flex; flex-direction: column; align-items: center; gap: 0.25rem; }
.held-card .card-face { width: 3rem; height: 4.3rem; }
.held-card .card-rank { font-size: 1.1rem; }
.held-card .card-suit { font-size: 0.95rem; }
.held-holder { font-size: 0.8rem; color: var(--muted); white-space: nowrap; }
.draw-counts { list-style: none; padding: 0; }
.draw-counts li { display: flex; justify-content: space-between; }
.btn-game-end { color: var(--danger); border-color: var(--danger); font-size: 0.9rem; }
/* Spectator: current card large, no interactive controls. */
.screen .card-face { width: 8rem; height: 11.5rem; }
.screen .card-rank { font-size: 3rem; }
.screen .card-suit { font-size: 2.6rem; }
.screen .btn-draw, .screen .btn-game-end, .screen .use-btn,
.screen .game-idle, .screen .presets-link { display: none; }
```

Check the existing top of `game.css` for the actual CSS variable names (`--muted`, `--danger`) and reuse whatever is defined there — do not invent new variables.

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test -p drinkinggame 2>&1 | tail -15`
Expected: ALL tests pass — 8 new integration tests plus every pre-existing db/http/render test.

- [ ] **Step 7: Lint, format, commit**

```bash
cargo clippy -p drinkinggame && cargo fmt
git add drinkinggame/src/game.rs drinkinggame/src/lib.rs drinkinggame/src/routes.rs \
  drinkinggame/templates/room.html drinkinggame/templates/screen.html \
  drinkinggame/assets/game.css drinkinggame/tests/http.rs
git commit -m "feat(drinks): ring of fire game routes with live SSE panel"
```

---

### Task 7: Presets pages

**Files:**
- Create: `drinkinggame/src/presets.rs`, `drinkinggame/templates/presets.html`, `drinkinggame/templates/preset_edit.html`
- Modify: `drinkinggame/src/lib.rs` (add `pub mod presets;`)
- Modify: `drinkinggame/src/render.rs` (preset row/option/form-row builders; refactor `game_idle_panel` to share options)
- Modify: `drinkinggame/src/routes.rs` (register `/presets` routes)
- Modify: `drinkinggame/assets/game.css`
- Test: `drinkinggame/tests/http.rs`

**Interfaces:**
- Consumes: `db::{list_presets, get_preset, insert_preset, update_preset, delete_preset}` (Task 3), `rules::{RuleEntry, parse_rules}` (Task 2), `cards` rank labels via `Card::rank_label` (Task 1), `routes::error_page` (existing, already `pub`).
- Produces:
  - `GET /presets` (auth), `POST /presets` (create-as-copy, form `name`, `source_id`), `GET /presets/{id}` (edit form), `POST /presets/{id}` (save, form `name` + `title_N`/`text_N`/`holdable_N` for N in 1..=13), `POST /presets/{id}/delete`
  - `render::preset_options(presets: &[RulePreset]) -> String`, `render::preset_rows(base_path: &str, presets: &[RulePreset]) -> String`, `render::preset_edit_rows(rules: &[RuleEntry]) -> String`

- [ ] **Step 1: Write the failing tests**

Render tests, appended inside `mod tests` in `drinkinggame/src/render.rs`:

```rust
#[test]
fn test_preset_rows_link_and_delete() {
    let html = preset_rows("/drinks", &[preset(3, "House <1>")]);
    assert!(html.contains(r#"href="/drinks/presets/3""#));
    assert!(html.contains("House &lt;1&gt;"));
    assert!(html.contains(r#"action="/drinks/presets/3/delete""#));
}

#[test]
fn test_preset_edit_rows_cover_all_13_ranks() {
    let html = preset_edit_rows(&crate::rules::standard_rules());
    for n in 1..=13 {
        assert!(html.contains(&format!(r#"name="title_{n}""#)));
        assert!(html.contains(&format!(r#"name="text_{n}""#)));
        assert!(html.contains(&format!(r#"name="holdable_{n}""#)));
    }
    assert!(html.contains("<legend>A</legend>"));
    assert!(html.contains("<legend>K</legend>"));
    // Holdables (5, 7) come back pre-checked.
    assert_eq!(html.matches("checked").count(), 2);
}
```

Integration tests, appended to `drinkinggame/tests/http.rs`:

```rust
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
    let loc = res.headers()[header::LOCATION].to_str().unwrap().to_string();
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
        let title = if r.rank == override_rank { new_title } else { &r.title };
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
    s.replace('%', "%25").replace('&', "%26").replace('+', "%2B").replace(' ', "+")
}

#[tokio::test]
async fn test_preset_save_and_delete() {
    let (app, pool) = test_app_with_pool().await;
    let cookie = login(&app, "alice", "1234").await;
    let res = post_form(&app, &cookie, "/presets", "name=House&source_id=1").await;
    let loc = res.headers()[header::LOCATION].to_str().unwrap().to_string();
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
    post_form(&app, &cookie, &format!("/room/{code}/game/start"), "preset_id=1").await;
    // Mutate Standard after the game started.
    post_form(&app, &cookie, "/presets/1", &edit_body("Standard", 1, "Tsunami")).await;
    // The running game still holds the snapshot.
    let room = drinkinggame::db::get_open_room(&pool, &code).await.unwrap();
    let game = drinkinggame::db::get_active_game(&pool, room.id).await.unwrap();
    let rules = drinkinggame::rules::parse_rules(&game.rules_json);
    assert_eq!(drinkinggame::rules::rule_for_rank(&rules, 1).title, "Waterfall");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p drinkinggame preset 2>&1 | head -20`
Expected: COMPILE ERROR — `preset_rows` and the routes don't exist.

- [ ] **Step 3: Implement render builders**

In `drinkinggame/src/render.rs`, add (and refactor `game_idle_panel` to call `preset_options` instead of its inline options loop — delete the inline `options` closure there):

```rust
pub fn preset_options(presets: &[RulePreset]) -> String {
    presets
        .iter()
        .map(|p| format!(r#"<option value="{}">{}</option>"#, p.id, html_escape(&p.name)))
        .collect()
}

/// <li> rows for the preset list page: edit link + delete form.
pub fn preset_rows(base_path: &str, presets: &[RulePreset]) -> String {
    presets
        .iter()
        .map(|p| {
            format!(
                r#"<li><a href="{base_path}/presets/{}">{}</a><form method="post" action="{base_path}/presets/{}/delete" onsubmit="return confirm('Delete this preset?')"><button class="btn-delete">Delete</button></form></li>"#,
                p.id,
                html_escape(&p.name),
                p.id,
            )
        })
        .collect()
}

/// One <fieldset> per rank for the edit form. Field names are rank-suffixed
/// (title_1..title_13 etc.) — the save handler reassembles them by rank.
pub fn preset_edit_rows(rules: &[crate::rules::RuleEntry]) -> String {
    rules
        .iter()
        .map(|r| {
            let label = Card { rank: r.rank, suit: crate::cards::Suit::Spades }.rank_label();
            let checked = if r.holdable { " checked" } else { "" };
            format!(
                r#"<fieldset class="rank-row"><legend>{label}</legend><input name="title_{rank}" value="{title}" maxlength="40" required><textarea name="text_{rank}" rows="2" maxlength="300" required>{text}</textarea><label class="hold-label"><input type="checkbox" name="holdable_{rank}"{checked}> Holdable</label></fieldset>"#,
                rank = r.rank,
                title = html_escape(&r.title),
                text = html_escape(&r.text),
            )
        })
        .collect()
}
```

- [ ] **Step 4: Implement templates and handlers**

Create `drinkinggame/templates/presets.html`:

```html
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Rule presets</title>
  <link rel="stylesheet" href="{{ base_path }}/assets/game.css">
</head>
<body>
  <h1>Rule presets</h1>
  <ul class="preset-list">{{ preset_rows|safe }}</ul>
  <h2>New preset</h2>
  <form method="post" action="{{ base_path }}/presets" class="preset-create">
    <input name="name" placeholder="Preset name" maxlength="40" required>
    <select name="source_id">{{ source_options|safe }}</select>
    <button type="submit">Create as copy</button>
  </form>
  <p><a href="{{ base_path }}/">Back home</a></p>
</body>
</html>
```

Create `drinkinggame/templates/preset_edit.html`:

```html
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Edit preset</title>
  <link rel="stylesheet" href="{{ base_path }}/assets/game.css">
</head>
<body>
  <h1>Edit preset</h1>
  <form method="post" action="{{ base_path }}/presets/{{ id }}" class="preset-edit">
    <input name="name" value="{{ name }}" maxlength="40" required>
    {{ rank_rows|safe }}
    <button type="submit">Save all 13 rules</button>
  </form>
  <p><a href="{{ base_path }}/presets">Back to presets</a></p>
</body>
</html>
```

Create `drinkinggame/src/presets.rs`:

```rust
//! Rule-preset pages: list, create-as-copy, edit, delete. Auth-required but
//! not owner-scoped — it's a friends app; anyone logged in may edit.

use askama::Template;
use axum::extract::{Form, Path, State};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Redirect};
use serde::Deserialize;
use std::collections::HashMap;

use crate::auth::PlayerSession;
use crate::db;
use crate::render;
use crate::routes::error_page;
use crate::rules::RuleEntry;
use crate::GameState;

#[derive(Template)]
#[template(path = "presets.html")]
struct PresetsTemplate {
    base_path: String,
    preset_rows: String,
    source_options: String,
}

#[derive(Template)]
#[template(path = "preset_edit.html")]
struct PresetEditTemplate {
    base_path: String,
    id: i64,
    name: String,
    rank_rows: String,
}

pub async fn presets_page(
    State(state): State<GameState>,
    PlayerSession(_player): PlayerSession,
) -> impl IntoResponse {
    let presets = db::list_presets(&state.pool).await;
    let tpl = PresetsTemplate {
        base_path: state.base_path.to_string(),
        preset_rows: render::preset_rows(&state.base_path, &presets),
        source_options: render::preset_options(&presets),
    };
    Html(tpl.render().unwrap())
}

#[derive(Deserialize)]
pub struct CreateForm {
    pub name: String,
    pub source_id: i64,
}

pub async fn create_preset(
    State(state): State<GameState>,
    PlayerSession(_player): PlayerSession,
    Form(form): Form<CreateForm>,
) -> axum::response::Response {
    let name = form.name.trim();
    if name.is_empty() || name.chars().count() > 40 {
        return error_page(&state, StatusCode::UNPROCESSABLE_ENTITY, "preset name must be 1-40 characters");
    }
    let Some(source) = db::get_preset(&state.pool, form.source_id).await else {
        return error_page(&state, StatusCode::NOT_FOUND, "no preset with that id");
    };
    match db::insert_preset(&state.pool, name, &source.rules_json).await {
        Ok(id) => Redirect::to(&format!("{}/presets/{id}", state.base_path)).into_response(),
        // UNIQUE name violation — the only insert error a user can cause.
        Err(_) => error_page(&state, StatusCode::CONFLICT, "a preset with that name already exists"),
    }
}

pub async fn edit_preset_page(
    State(state): State<GameState>,
    PlayerSession(_player): PlayerSession,
    Path(id): Path<i64>,
) -> axum::response::Response {
    let Some(preset) = db::get_preset(&state.pool, id).await else {
        return error_page(&state, StatusCode::NOT_FOUND, "no preset with that id");
    };
    let rules = crate::rules::parse_rules(&preset.rules_json);
    let tpl = PresetEditTemplate {
        base_path: state.base_path.to_string(),
        id: preset.id,
        name: preset.name,
        rank_rows: render::preset_edit_rows(&rules),
    };
    Html(tpl.render().unwrap()).into_response()
}

pub async fn save_preset(
    State(state): State<GameState>,
    PlayerSession(_player): PlayerSession,
    Path(id): Path<i64>,
    Form(form): Form<HashMap<String, String>>,
) -> axum::response::Response {
    let name = form.get("name").map(|s| s.trim()).unwrap_or("");
    if name.is_empty() || name.chars().count() > 40 {
        return error_page(&state, StatusCode::UNPROCESSABLE_ENTITY, "preset name must be 1-40 characters");
    }
    let mut rules = Vec::with_capacity(13);
    for rank in 1..=13u8 {
        let title = form.get(&format!("title_{rank}")).map(|s| s.trim()).unwrap_or("");
        let text = form.get(&format!("text_{rank}")).map(|s| s.trim()).unwrap_or("");
        if title.is_empty() || text.is_empty() {
            return error_page(&state, StatusCode::UNPROCESSABLE_ENTITY, "every rank needs a title and text");
        }
        rules.push(RuleEntry {
            rank,
            title: title.to_string(),
            text: text.to_string(),
            // Unchecked checkboxes are simply absent from the form body.
            holdable: form.contains_key(&format!("holdable_{rank}")),
        });
    }
    let rules_json = serde_json::to_string(&rules).expect("rules serialize");
    match db::update_preset(&state.pool, id, name, &rules_json).await {
        Ok(true) => Redirect::to(&format!("{}/presets", state.base_path)).into_response(),
        Ok(false) => error_page(&state, StatusCode::NOT_FOUND, "no preset with that id"),
        Err(_) => error_page(&state, StatusCode::CONFLICT, "a preset with that name already exists"),
    }
}

pub async fn delete_preset_handler(
    State(state): State<GameState>,
    PlayerSession(_player): PlayerSession,
    Path(id): Path<i64>,
) -> axum::response::Response {
    // Deleting is always allowed — running games hold snapshots, and the
    // migration guard recreates Standard on next deploy if it goes missing.
    db::delete_preset(&state.pool, id).await;
    Redirect::to(&format!("{}/presets", state.base_path)).into_response()
}
```

Add `pub mod presets;` to `drinkinggame/src/lib.rs`. Register in `routes.rs` `router()`:

```rust
        .route("/presets", get(crate::presets::presets_page).post(crate::presets::create_preset))
        .route("/presets/{id}", get(crate::presets::edit_preset_page).post(crate::presets::save_preset))
        .route("/presets/{id}/delete", post(crate::presets::delete_preset_handler))
```

Append CSS to the Ring of Fire section of `drinkinggame/assets/game.css`:

```css
.preset-list { list-style: none; padding: 0; width: 100%; max-width: 420px; }
.preset-list li { display: flex; justify-content: space-between; align-items: center; gap: 0.5rem; }
.preset-create, .preset-edit { display: flex; flex-direction: column; gap: 0.5rem; width: 100%; max-width: 420px; }
.rank-row { border: 1px solid var(--muted); border-radius: 0.5rem; display: flex; flex-direction: column; gap: 0.4rem; }
.hold-label { font-size: 0.9rem; color: var(--muted); }
.btn-delete { color: var(--danger); border-color: var(--danger); font-size: 0.85rem; }
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p drinkinggame 2>&1 | tail -15`
Expected: ALL tests pass (2 new render + 5 new integration tests included).

- [ ] **Step 6: Lint, format, commit**

```bash
cargo clippy -p drinkinggame && cargo fmt
git add drinkinggame/src/presets.rs drinkinggame/src/render.rs drinkinggame/src/routes.rs \
  drinkinggame/src/lib.rs drinkinggame/templates/presets.html \
  drinkinggame/templates/preset_edit.html drinkinggame/assets/game.css drinkinggame/tests/http.rs
git commit -m "feat(drinks): rule preset pages (list, copy, edit, delete)"
```

---

### Task 8: Full verification, manual browser check, docs

**Files:**
- Modify: `CLAUDE.md` (worktree root — the drinks paragraph)
- Modify: `docs/superpowers/specs/2026-07-29-ring-of-fire-design.md` (status line)

- [ ] **Step 1: Full workspace verification**

Run from the worktree root and quote the output:

```bash
cargo test 2>&1 | tail -20        # whole workspace, not just -p drinkinggame
cargo clippy --workspace 2>&1 | tail -5
cargo fmt --check
```

Expected: all tests pass, clippy clean, fmt clean.

- [ ] **Step 2: Manual browser verification (REQUIRED — do not skip)**

```bash
cargo run -p drinkinggame   # standalone on :3001
```

Then verify in a real browser (the executing agent should use its browser tooling; if unavailable, STOP and ask the human partner to check):
1. Two windows logged in as different players, same room; spectator screen (`/room/CODE/screen`) in a third.
2. Start Ring of Fire with the Standard preset → panel appears in all three windows without reload.
3. Draw from window A → card face, rule title/text, "A drew", remaining count and draw totals update everywhere; card is large on the spectator screen.
4. Draw until a 5 or 7 appears → held strip shows on all screens; **Use button visible ONLY in the holder's window, absent in the other phone window and the spectator screen**.
5. Use the held card → announcement broadcast, card leaves the strip.
6. Refresh window B mid-game → identical state re-renders (DB-backed recovery).
7. End game early → summary + Start button return; tracker +1 Drink / Undo buttons still work throughout.
8. Phone-sized viewport (~390px): panel, card, held strip and presets edit form all usable.
9. `/presets`: create a copy, edit a title, save, delete it.

- [ ] **Step 3: Update docs**

In the worktree root `CLAUDE.md`, extend the `/drinks` sentence to mention the game, e.g.: "`/drinks` is the `drinkinggame` crate (own DB, own name+PIN sessions, SSE leaderboards, Ring of Fire card game with server-side rule presets at `/drinks/presets`) nested via `nest_service` in `main.rs`; its templates do NOT extend `base.html` (recorded exception)."

In `docs/superpowers/specs/2026-07-29-ring-of-fire-design.md`, change `**Status:** Approved` to `**Status:** Implemented`.

- [ ] **Step 4: Commit**

```bash
git add CLAUDE.md docs/superpowers/specs/2026-07-29-ring-of-fire-design.md
git commit -m "docs: ring of fire implemented — update CLAUDE.md and spec status"
```

---

## Self-Review Notes (already applied)

- **Spec coverage:** shared deck + tap-to-draw (T6), no turns + per-player counts (T4/T5), holdable multi-holder cards with holder-only Use (T4–T6), deck-empty auto-end + early end + summary (T6), presets CRUD with Standard seed and snapshot-on-start (T3/T7), one-active-game constraint (T3/T4), DB-backed recovery (page-load server render, T6 step 4), pure-CSS card faces (T5), SSE deltas over the existing hub (T5/T6), typed errors (T4), tracker buttons untouched (T6 templates keep them), unit + integration + manual testing (T1–T8). Out-of-scope items from the spec are not implemented anywhere — correct.
- **`spend_draw` was re-scoped mid-plan** to take `game_id`, preventing spends against draws from ended games; Tasks 4 and 6 both use the 4-arg form.
- **Known accepted quirks:** the game-over summary is broadcast-only (a late page refresh shows the idle panel, not the summary); spectator screens show the panel read-only via CSS hiding; announcement lines are transient by design.





