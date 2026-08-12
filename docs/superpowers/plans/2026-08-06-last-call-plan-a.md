# Last Call — Plan A: the component library

> **For agentic workers:** REQUIRED SUB-SKILLS: `plan-economics` (this repo's
> task classes, review policy and plan sizing) then
> superpowers:subagent-driven-development to execute task-by-task.

**Goal:** Build the object model, the design tokens and every rendering
component Last Call needs — the five card primitives with their text rules, the
player plaque and its five states, the hand strip, the deck stack and the
discard slot — each satisfying the §7.8 DOM contract, and each verified by unit
tests.

**Architecture:** `src/last_call.rs` is a pure state machine in the shape of
`three_man.rs` — no I/O, no SQL, no RNG. `src/lc_render.rs` builds fragments as
formatted strings the way `render.rs` already does, emitting **deck class names
and never hex colours**, so `assets/lastcall.css` owns colour and the renderers
own markup. Public components take a projected `PublicView` / `PublicSeat`
rather than `&LastCallState` (spec §3.4) — nothing structural should let a
renderer reach into `players[i].hand` while building markup that gets broadcast.

**Slice:** Plan A of four for slice 1 (spec §10), in the order **A → A-vis → A2
→ B**.

**Nothing in this plan is viewable in a browser.** There is no route, no
template and no page at the end of it — only `lastcall.css`, a module of
builders, and a test suite. That is deliberate: the acceptance is
`./scripts/verify.sh`, and there are **no browser checkpoints**, because there
is nothing to look at. **Plan A-vis is where this becomes visible**, via the
`GET /lastcall/preview` gallery, and it comes immediately next precisely so a
token or a text rule can be corrected while the only consumer is a fixture page.

**The four-plan chain, end to end:**

1. **Plan A — the component library** *(this plan)*. Types, `PublicView`, the
   adversarial catalog, the shared `preview_state()` fixture builder,
   `lastcall.css` tokens and §7.6 scene primitives, and `lc_render.rs`'s
   components to the §7.8 contract. *Deployable:* a tested component library,
   verified by unit tests rather than by eye.
2. **Plan A-vis — motion and the style guide.** The §7.7 motion library and
   flight helper, then `GET /lastcall/preview` — the route, the `PublicView`
   fixtures and the gallery. *Deployable:* a URL that shows the whole visual
   vocabulary, which is Module Spec G's step-1 done-when verbatim and then some.
3. **Plan A2 — the game wiring.** `last_call` as a third `games.kind`, the setup
   form, the entry redirect, the F.1 phone shell, the private hand route and the
   SSE contract. Every Class C task in the slice lives there.
4. **Plan B — the felt surfaces.** `lc_screen.html`, the D.2 seat-ring angle
   layout that positions **this plan's** plaques around the felt, the
   `/room/{code}/screen` kind branch, `GET …/lastcall/table`, and the F.3 mini
   table. **Plan B assembles components and authors none** — §7.6's
   component/positioning split already put the plaque, hand strip, deck stack
   and discard slot here.

**The component / positioning split, because it decides every module.** A
*component* renders from its own data and ships in Plan A; its *placement*
depends on table state and ships in Plan B. Drawing the line anywhere else
breaks §7.7: `lc-shake`, `lc-hp-flash` and `lc-pulse` all target the plaque, so
authoring those animations in Plan A-vis while the plaque arrived in Plan B
would leave a task animating a component that does not exist.

**`PublicView` ships here even though nothing consumes it yet.** It is a
projection function and a struct — cheap to write, unit-tested here, and the
type every renderer in the three later plans is built against. Deferring it
would mean writing those renderers against `&LastCallState` and refactoring all
of them later, which spec §3.4 names as the exact cost it exists to avoid.

**`preview_state()` is defined here, not in Plan A-vis.** Spec §8 makes it one
shared builder, explicitly **not** `#[cfg(test)]`: Task 3's plaque tests and Plan
A-vis's preview route render the same eight-seat state, so a test failure and a
visual regression cannot disagree about what the fixture is. Plan A-vis consumes
it and defines no fixtures of its own.

**Size note.** Three tasks is under `plan-economics`'s 4–6 guide. That is the
result of splitting the visual vocabulary at the seam where nothing flows back:
Tasks 1–3 produce, Plan A-vis consumes. Task 3 is the largest brief in the
series (~430 lines, 18 tests) because it merges the card primitives and the
table components; splitting it would make four tasks with no natural boundary.
The file runs long against the ~800–1,200-line guide for the reason
`plan-economics` §5 allows: most of it is verbatim CSS token blocks, the 20-row
adversarial catalog with its character counts, the §7.8 contract table, type
signatures and test tables with expected values — the material that lets each
task run on a cheap model without inventing a class name.

---

## Global Constraints

Every task's requirements implicitly include this section.


### The §7.8 component contracts — verbatim, and binding on every task

Building templates before wiring only pays off if the wiring knows what it is
wiring *to*. Each component declares a DOM contract — the same role the
Interfaces block plays for a plan task. **Plan A's markup must match this table
exactly.**

| Component | Root | Requires | Exposes | Motion anchor | Filled by |
| --- | --- | --- | --- | --- | --- |
| Hand region | `#lc-hand` | `data-seq` | `data-count` | `hand` | `GET …/lastcall/hand` (A2) |
| CardFace | `.lc-cardface[data-card-id]` | `data-deck`, `data-cost` | `data-expandable` | — | within Hand region |
| CardPip | `.lc-pip` | `data-deck`, `data-cost` | — | — | within CardFace |
| CardMini | `.lc-mini[data-card-id]` | `data-deck`, `data-cost` | — | — | armed column (slice 2) |
| CardBack | `.lc-back` | `data-deck`, `data-size` | — | — | hand strips, piles, flights |
| CardDot | `.lc-dot` | `data-deck` | — | — | mini-table flights |
| PlayerPlaque | `.lc-plaque[data-seat]` | `data-decks`, `data-hp`, `data-status` | `data-hand-size` | `plaque-seat-{n}` | `LcPublic` SSE (A2) |
| HandStrip | `.lc-handstrip` | `data-hand-size`, `data-decks` | — | — | within PlayerPlaque |
| DeckStack | `.lc-deckstack[data-deck]` | `data-count` | `data-low`, `data-empty` | `deck-{deck}` | `LcPublic` SSE (A2) |
| DiscardSlot | `.lc-discard` | `data-count` | — | `discard` | `LcPublic` SSE (A2) |
| PhaseBanner | `#lc-banner` | `data-beat`, `data-round` | — | — | `LcPublic` SSE (A2) |
| BeatTimer | `#lc-beat-timer` | `data-duration-ms`, `data-elapsed-ms` | — | — | `LcPublic` SSE (A2) |
| Felt scene | `#lc-felt` | — | — | `felt` | static |
| Flight layer | `#lc-flights` | — | — | — | motion helper (§7.7) |

**The contract is structure, never behaviour.** It says `[data-card-id]` exists
and is the click target; it does **not** say that tapping arms the card. **If an
`hx-post` or `hx-get` path appears anywhere in Plan A, the line has been
crossed** — that is slice 2 and 3 work. If a task starts describing what an
interaction *does*, it has drifted out of scope.

### Motion anchors (§7.8.1) — this plan's markup carries them

The §7.7 helper computes `--dx`/`--dy` from two bounding rects, so **every
flight source and destination needs a stable, resolvable name in Plan A's
markup**, even though nothing fires a flight until slice 3. The attribute is
`data-flight-anchor="<name>"` and the complete name set is:

```
deck-beer  deck-cider  deck-wine  deck-liquor  deck-soft
discard
plaque-seat-0 … plaque-seat-7
hand
felt
```

This plan emits `data-flight-anchor` on the plaque, the deck stack and the
discard slot (Task 3); Plan A-vis emits it on the felt and the hand region, and
carries the test that resolves every name on the preview page. Markup without
anchors means slice 3 rewrites every template, which is the single most
expensive thing this staging exists to prevent.

### Repo rules that bind here

- **No SQL, and no new db function.** This plan reads and writes nothing. If a
  task believes it needs `db.rs`, it has misread the plan.
- **No migration**, and `cargo sqlx prepare` is not needed — the `drinkinggame`
  crate uses runtime-checked sqlx queries and has no `.sqlx` cache entries
  (CLAUDE.md), and this plan adds no query.
- **This plan adds no template.** Every rule lives in `assets/lastcall.css`
  under a named section comment. Never nest `/*` inside a CSS comment — the
  guard test in Task 2 exists because that bug silently dropped `.card-big`
  once.
- **This plan ships no JavaScript and no template.** The
  `DOMContentLoaded` + `htmx:afterSwap` + double-injection-guard rule
  (CLAUDE.md) therefore never comes up here; it binds Plan A-vis's
  `lc_motion.js`.
- **`palette.js` / `base.html` nav are not touched.** Those apply to new
  *portfolio* sections; Last Call lives inside the already-registered `/drinks`
  mount.

### Deck constants — DDv2 §3.1–3.2, spec §4

Pulls are a **deck constant, not a volume**. `pulls_max = deck.pulls()`. The
`container` field is a free-text label and **never affects `pulls_max`** — a
Beer vessel is 8 pulls whether the tin is 50cl or 25cl.

| Deck | slug | pulls | cost spread | role |
| --- | --- | --- | --- | --- |
| Beer | `beer` | 8 | 1–2 | Attrition |
| Cider | `cider` | 10 | 1–3 | Trickster |
| Wine | `wine` | 6 | 2–3 | Control |
| Liquor | `liquor` | 4 | 2–3 | Burst |
| Soft | `soft` | 6 | 1–2 | Support |

Starting HP is **15** for everyone (DDv2 §2.4). Handicap multiplies card cost in
pulls, **rounds up**, and touches nothing else (DDv2 §11).

### Deck colour ramps — spec §7.2, design README

The only taxonomy. No card-type palette, no per-player colour. *Fill* for solid
areas, *ink* for anything on the dark ground; they differ for Wine only.

| Deck | Fill | Ink | Text on fill |
| --- | --- | --- | --- |
| Beer | `#FFB570` | `#FFB570` | `#14101D` |
| Cider | `#B48EF7` | `#B48EF7` | `#14101D` |
| Wine | `#8B2F4A` | `#D4657F` | `#F2EEF8` |
| Liquor | `#F7768E` | `#F7768E` | `#14101D` |
| Soft | `#6FB6FF` | `#6FB6FF` | `#0D1620` |

**Renderers emit deck classes, never hex.** Task 2 owns colour; Task 3 owns
markup. `lc_render.rs` emits `lc-deck-wine`; `lastcall.css` binds `--lc-fill` /
`--lc-ink` / `--lc-on-fill` / `--lc-grid` / the four `--lc-ink-NN` alphas on that
class. Task 3's tests assert this by rejecting any `#` in renderer output.

Beat hues: Draw amber, Diplomacy mint, Lock violet, Reveal azure, Resolve rose.
**Deal has no hue in the bundle** — it inherits Draw's amber (judgment call,
recorded here so it is not re-litigated).

### Surfaces and text — design README

| Token | Value | Use |
| --- | --- | --- |
| page | `#0B0910` | Document ground |
| device | `#0E0C14` | Phone body |
| card / panel | `#16121F` | Plaques, deck rows |
| card alt | `#17141F` | List rows, secondary buttons |
| raised card | `#251F35` | Unfocused wheel cards |
| focused card | `#2E2742` | The focused wheel card |
| card back | `#1B1628` | All card backs and deck stacks |

Text: primary `#F2EEF8`, body `#CDC6DD`, secondary `#A79FBB`, label `#8D87A0`,
faint `#6A6480`. Status accents: mint `#4FD6A8`, amber `#FFB570`, rose
`#F7768E`, azure `#6FB6FF`, violet `#B48EF7`. Hairlines `rgba(242,238,248,.10)`
to `rgba(242,238,248,.28)`.

Radii: 3px badges, 5px pips, 6px small cards/secondary buttons, 8px primary
buttons and panels, 10px plaques, 12–14px card faces.
Elevation: `0 3px 0 rgba(5,3,10,.5), 0 8px 16px rgba(5,3,10,.42)` (small cards),
`0 6px 0 rgba(5,3,10,.6), 0 22px 40px rgba(5,3,10,.55)` (focused card). The hard
offset is the deck-of-cards look — do not replace it with a blur-only shadow.
Motion: 130ms taps, 190ms state changes, 280ms position changes, all on
`cubic-bezier(.2,.8,.3,1)`.

### §7.5 text-handling thresholds

| Title length | Size | Class |
| --- | --- | --- |
| ≤ 14 chars | 30px (the authored size) | `lc-title-lg` |
| 15–24 chars | 24px | `lc-title-md` |
| > 24 chars | 20px | `lc-title-sm` |

Title clamped to 2 lines; body Space Grotesk 400/15px/1.35 clamped to 3 lines;
at most 3 keyword chips then a `+n` chip; CardMini name clamped to 2 lines.
Truncation is decided **server-side from the string**, so it is deterministic
and unit-testable — which is why the ramp is expressed in characters rather than
measured width.

### Routes added by this plan

Three, all public and unguarded. Routes are written unprefixed —
`nest_service` strips the `/drinks` mount, and only *generated URLs*
interpolate `base_path`.

| Method | Path | Task |
| --- | --- | --- |
| GET | `/assets/lastcall.css` | 2 |

That is the only one. `/assets/lc_motion.js` and `/lastcall/preview` are Plan
A-vis's; every `/room/…` route is Plan A2's.

**Verification for every task:** `./scripts/verify.sh` — all green, output
quoted in the report.

**Browser checkpoints:** **none.** This plan produces no page, so there is
nothing to open. The acceptance is the test suite, and the single plan-end
whole-diff review on the most capable model covers all three tasks (they are
Class A/B and carry no per-task reviewer). Plan A-vis is where the design is
first seen.

---

---
### Task 1: `last_call.rs` object model, `PublicView`, and the adversarial catalog

**Class:** B (logic, tests specified below)

**Why this class:** every claim here is a pure function over pure data with the
expected value written into the plan — the pull table, the handicap rounding,
the serde round-trip, the projection and the catalog's text bands are all
decidable by `cargo test`.

**Files:**
- Create: `drinkinggame/src/last_call.rs`
- Create: `drinkinggame/src/lc_cards.rs`
- Modify: `drinkinggame/src/lib.rs` (add `pub mod last_call;` and
  `pub mod lc_cards;` to the module list, alphabetical)
- Test: `drinkinggame/src/last_call.rs` and `drinkinggame/src/lc_cards.rs`
  `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: nothing. `last_call.rs` imports only `serde`; `lc_cards.rs` imports
  only `last_call`'s types.
- Produces (every later task and both later plans build against these exact
  signatures):

```rust
// last_call.rs
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Deck { Beer, Cider, Wine, Liquor, Soft }

impl Deck {
    pub const ALL: [Deck; 5] = [Deck::Beer, Deck::Cider, Deck::Wine, Deck::Liquor, Deck::Soft];
    pub fn pulls(self) -> u8;             // 8 / 10 / 6 / 4 / 6
    pub fn slug(self) -> &'static str;    // "beer" | "cider" | "wine" | "liquor" | "soft"
    pub fn label(self) -> &'static str;   // "BEER" | "CIDER" | "WINE" | "LIQUOR" | "SOFT"
    pub fn from_slug(s: &str) -> Option<Deck>;
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Beat { #[default] Draw, Deal, Diplomacy, Lock, Reveal, Resolve }

impl Beat {
    pub const ORDER: [Beat; 6];
    pub fn index(self) -> u8;             // 1..=6
    pub fn label(self) -> &'static str;   // "DRAW" .. "RESOLVE"
    pub fn slug(self) -> &'static str;    // "draw" .. "resolve" — the `data-beat` value
    pub fn hue(self) -> &'static str;     // "amber","amber","mint","violet","azure","rose"
    pub fn next(self) -> Beat;            // wraps Resolve -> Draw
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CardKind { Atk, Buff, Curse, Util, Reaction }

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Status { Alive, Eliminated }

impl Status { pub fn slug(self) -> &'static str; }   // the `data-status` value

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Vessel { pub deck: Deck, pub pulls_max: u8, pub pulls_left: u8, pub container: String }

/// `title` is NOT in DDv2 §1's object model — card titles are shown on
/// CardFace and CardMini and have to live somewhere, so they are folded into
/// `Card` rather than kept in a parallel lookup. Record that in a comment.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Card {
    pub id: String, pub deck: Deck, pub kind: CardKind, pub cost: u8,
    pub targets: String, pub title: String, pub text: String,
    pub keywords: Vec<String>, pub duration: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct LcPlayer {
    pub seat: usize, pub player_id: i64, pub name: String, pub hp: i32,
    pub handicap_pct: u16, pub vessels: Vec<Vessel>, pub hand: Vec<Card>,
    pub armed: Vec<Card>, pub locked: bool, pub drawing: bool,
    pub draws_this_round: u16, pub tabs: Vec<String>, pub status: Status,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Play {
    pub card: Card, pub source_seat: usize, pub target: Option<usize>,
    pub paid_from: Deck, pub order_key: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Effect {
    pub source_play: u32, pub subject: usize, pub op: String,
    pub magnitude: i32, pub expires_round: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct LastCallState {
    pub players: Vec<LcPlayer>, pub round: u32, pub beat: Beat,
    pub first_seat: usize, pub rng_seed: u64, pub plays: Vec<Play>,
    pub effects: Vec<Effect>, pub discards: Vec<Card>,
    pub deck_counts: Vec<(Deck, u16)>, pub seq: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PublicVessel { pub deck: Deck, pub pulls_left: u8, pub pulls_max: u8 }

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PublicSeat {
    pub seat: usize, pub player_id: i64, pub name: String, pub hp: i32,
    pub status: Status, pub vessels: Vec<PublicVessel>,
    pub hand_len: usize, pub locked: bool, pub drawing: bool,
    /// Cards drawn this round — the plaque's deck-tinted badge (D.1 row 2).
    /// Projected from `LcPlayer::draws_this_round`, which the Draw beat sets
    /// in slice 3 and nothing sets here.
    pub draws: u16,
}

impl PublicSeat { pub fn decks(&self) -> Vec<Deck>; }   // one per vessel, in order

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PublicView {
    pub seats: Vec<PublicSeat>, pub round: u32, pub beat: Beat,
    pub first_seat: usize, pub deck_counts: Vec<(Deck, u16)>,
    pub discard_count: usize, pub revealed: Vec<Play>, pub seq: u64,
}

/// Shared runtime fixture builder (spec §8) — NOT `#[cfg(test)]`. Task 3's
/// plaque tests and Plan A-vis's preview route render the same eight-seat
/// state, so a test failure and a visual regression cannot disagree about
/// what the fixture is.
pub fn preview_state() -> LastCallState;

#[derive(Debug, PartialEq, Eq)]
pub enum LcError { NotSeated, BadHandicap, NotImplemented }

pub const STARTING_HP: i32 = 15;          // DDv2 §2.4, TBD-1
pub const MAX_SEATS: usize = 8;           // DDv2 §2.1 (2–8)
pub const HANDICAP_MIN_PCT: u16 = 25;
pub const HANDICAP_MAX_PCT: u16 = 300;
/// Under this many cards a DeckStack count turns amber (`data-low`).
pub const DECK_LOW_THRESHOLD: u16 = 5;

/// Rounds UP. DDv2 §11.
pub fn pull_cost(cost: u8, handicap_pct: u16) -> u8;

impl LastCallState {
    pub fn new(members: Vec<(i64, String)>, rng_seed: u64) -> Self;
    pub fn to_json(&self) -> String;
    pub fn from_json(s: &str) -> Self;
    pub fn seat_of(&self, player_id: i64) -> Option<usize>;
    pub fn add_player(&mut self, player_id: i64, name: &str);
    pub fn set_vessel(&mut self, player_id: i64, deck: Deck, container: &str) -> Result<(), LcError>;
    pub fn set_handicap(&mut self, target_id: i64, handicap_pct: u16) -> Result<(), LcError>;
    pub fn public_view(&self) -> PublicView;
    // Stubbed beat transitions — final signatures, bodies return NotImplemented.
    pub fn arm(&mut self, player_id: i64, card_id: &str) -> Result<(), LcError>;
    pub fn disarm(&mut self, player_id: i64, card_id: &str) -> Result<(), LcError>;
    pub fn lock_in(&mut self, player_id: i64) -> Result<(), LcError>;
    pub fn advance_beat(&mut self) -> Result<(), LcError>;
    pub fn resolve(&mut self) -> Result<(), LcError>;
}

// lc_cards.rs
pub struct CardDef {
    pub id: &'static str, pub deck: Deck, pub kind: CardKind, pub cost: u8,
    pub targets: &'static str, pub title: &'static str, pub text: &'static str,
    pub keywords: &'static [&'static str],
}
pub const CATALOG: [CardDef; 20];
pub fn deck_cards(deck: Deck) -> Vec<Card>;
pub fn card_by_id(id: &str) -> Option<Card>;
```

- [ ] **Step 1: Write `last_call.rs`'s types and constants**

Mirror `three_man.rs` in shape: a module doc comment stating "pure state
machine, no I/O, no SQL, no RNG", then the enums, then the structs, then one
`impl LastCallState`, then `#[cfg(test)] mod tests`.

Two exactness points that are not derivable:

1. `from_json` mirrors `ThreeManState::from_json` — it **`expect`s**, it does
   not fall back to `Default`:

```rust
pub fn to_json(&self) -> String {
    serde_json::to_string(self).expect("LastCallState is always serializable")
}

/// Deserializes a snapshot produced by `to_json`. Only ever called on this
/// engine's own output, so a parse failure is a programming error.
///
/// INVARIANT this creates: every writer must pass `Some(&st.to_json())` to
/// `db::start_game` (Plan A2), because every reader does
/// `from_json(game.state_json.as_deref().unwrap_or_default())` and `""` is
/// not valid JSON.
pub fn from_json(s: &str) -> Self {
    serde_json::from_str(s).expect("valid LastCallState JSON")
}
```

2. `LastCallState` derives `Default` (needed for cheap test construction and for
   `Beat: Default`), but `from_json` never uses it.

`pull_cost` is integer arithmetic — no floats anywhere in this module:

```rust
/// Handicap is a percentage (100 = no handicap). Rounds UP, per DDv2 §11.
/// Integer maths on purpose: a float handicap would let a form field carry
/// NaN/inf into the state blob and break both serde equality and `ceil()`.
pub fn pull_cost(cost: u8, handicap_pct: u16) -> u8 {
    ((cost as u32 * handicap_pct as u32 + 99) / 100) as u8
}
```

`set_handicap` rejects anything outside `HANDICAP_MIN_PCT..=HANDICAP_MAX_PCT`
with `LcError::BadHandicap`, and an unknown `target_id` with
`LcError::NotSeated`. Both it and `set_vessel` bump `self.seq`.

`new(members, rng_seed)` seats members in the order given, `seat` = index,
`hp = STARTING_HP`, `handicap_pct = 100`, empty vessels/hand/armed/tabs,
`locked = false`, `drawing = false`, `status = Status::Alive`; `round = 1`,
`beat = Beat::Draw`, `first_seat = 0`, `seq = 0`, and `deck_counts` initialized
from `Deck::ALL` at `0` (settable but never set by this slice, spec §4.1).

`add_player` mirrors `ThreeManState::add_player`: no-op if already seated,
otherwise push at `seat = players.len()`. It does not enforce `MAX_SEATS` —
that is a Plan B seat-ring layout concern; the constant is exported for it.

`locked`, `drawing` and `draws_this_round` are set by the loop and never by this
slice. They exist now because Task 3 renders them (`.is-locked`, `.is-drawing`,
the `.lc-draws` badge) and Plan A-vis animates the first two; adding them later would
mean re-cutting `PublicSeat`, which Plan A2 and Plan B both build against. This
is spec §4.1's "settable but never set" rule — the fields are real, the loop that
moves them is slice 3.

- [ ] **Step 2: `set_vessel` deals the placeholder hand**

Nothing in slice 1 deals cards — the Draw beat is slice 3 — so registering a
drink seeds the hand, which is what lets Plan A2's deployable ("a player sees
their own hand") exist at all:

```rust
/// Registers the player's drink. `pulls_max` is a DECK constant (DDv2 §3.2);
/// `container` is a free-text label and never affects it.
///
/// Slice-1 stub deal: the vessel also seeds the player's hand with that
/// deck's placeholder cards, because no Draw beat exists yet to do it. The
/// Draw beat replaces this in slice 3.
pub fn set_vessel(&mut self, player_id: i64, deck: Deck, container: &str) -> Result<(), LcError> {
    let Some(seat) = self.seat_of(player_id) else { return Err(LcError::NotSeated) };
    let p = &mut self.players[seat];
    p.vessels.retain(|v| v.deck != deck);
    p.vessels.push(Vessel {
        deck,
        pulls_max: deck.pulls(),
        pulls_left: deck.pulls(),
        container: container.to_string(),
    });
    for card in crate::lc_cards::deck_cards(deck) {
        if !p.hand.iter().any(|c| c.id == card.id) {
            p.hand.push(card);
        }
    }
    self.seq += 1;
    Ok(())
}
```

Re-registering the same deck replaces the vessel (pulls reset) and adds no
duplicate cards. Registering a *second* deck adds a second vessel and that
deck's cards — the two-deck case the README calls normal.

- [ ] **Step 3: Stub the beat transitions**

Final signatures, bodies `Err(LcError::NotImplemented)`, parameters named with a
leading underscore so nothing warns. Above them: *"Slice 1 defines the shape;
slice 3 (the loop) fills these in. The object model is expensive to change
later; transitions are not."*

- [ ] **Step 4: Write `public_view()` — the projection (spec §3.4)**

The whole point: a public renderer must be *unable* to reach private state, not
merely trusted not to. And per spec §3.4's last paragraph, the preview page
renders public components *through* this projection, so a missing field becomes
a compile error in Plan A-vis rather than a discovery in Plan A2.

```rust
/// Projects to exactly what D.1/D.3 and F.2 legitimately display. Card
/// identity survives only for plays already revealed — before beat Reveal,
/// `revealed` is empty, so a broadcast fragment cannot contain an unrevealed
/// card by construction. `armed` is never projected as a list or a count:
/// DDv2 §6.3 is "show only a lock tick per seat", which is `locked`.
pub fn public_view(&self) -> PublicView {
    PublicView {
        seats: self.players.iter().map(|p| PublicSeat {
            seat: p.seat,
            player_id: p.player_id,
            name: p.name.clone(),
            hp: p.hp,
            status: p.status,
            vessels: p.vessels.iter().map(|v| PublicVessel {
                deck: v.deck, pulls_left: v.pulls_left, pulls_max: v.pulls_max,
            }).collect(),
            hand_len: p.hand.len(),
            locked: p.locked,
            drawing: p.drawing,
            draws: p.draws_this_round,
        }).collect(),
        round: self.round,
        beat: self.beat,
        first_seat: self.first_seat,
        deck_counts: self.deck_counts.clone(),
        discard_count: self.discards.len(),
        revealed: match self.beat {
            Beat::Reveal | Beat::Resolve => self.plays.clone(),
            _ => Vec::new(),
        },
        seq: self.seq,
    }
}
```

- [ ] **Step 5: Write `lc_cards.rs` — 20 placeholder cards, deliberately adversarial**

Spec §9: the catalog is **deliberately adversarial, not tidy**. It must contain
at least one title in each band of the §7.5 ramp (≤14, 15–24, >24 characters),
one body that overflows three lines, one card with no keywords and one with six.
*"If every stub title is short, the 24px and 20px branches are exercised only by
synthetic test fixtures and never by anything rendered — which is how the ramp
reaches production untested."*

Costs stay inside each deck's spread. Character counts are given because they
are the point, and Step 7 asserts them rather than trusting the comment.

```rust
const KW6: &[&str] = &["burst", "loud", "public", "delayed", "stacking", "reactive"];
const KW3: &[&str] = &["slow", "control", "single"];
const NONE: &[&str] = &[];

/// A 149-character body — over BODY_CLAMP_CHARS (108), so it is the one card
/// that proves the 3-line clamp and the `data-expandable` marking against
/// rendered content rather than a test fixture.
const LONG_BODY: &str = "Placeholder. A slow, expensive problem that takes several \
lines to explain properly, which is exactly the point of it existing in a \
catalog that is otherwise far too tidy.";

pub const CATALOG: [CardDef; 20] = [
    // Beer — attrition, costs 1–2
    CardDef { id: "beer-01", deck: Deck::Beer, kind: CardKind::Atk, cost: 1, targets: "one", keywords: NONE,
              title: "Nudge",                      /* 5  -> lg */ text: "Placeholder. A small, boring hit." },
    CardDef { id: "beer-02", deck: Deck::Beer, kind: CardKind::Atk, cost: 2, targets: "one", keywords: KW3,
              title: "Grind",                      /* 5  -> lg */ text: "Placeholder. A slightly less small hit." },
    CardDef { id: "beer-03", deck: Deck::Beer, kind: CardKind::Buff, cost: 1, targets: "self", keywords: NONE,
              title: "Second Wind",                /* 11 -> lg */ text: "Placeholder. You feel marginally better." },
    CardDef { id: "beer-04", deck: Deck::Beer, kind: CardKind::Util, cost: 2, targets: "self", keywords: NONE,
              title: "Top Up, Then Top Up Again",  /* 25 -> sm */ text: "Placeholder. Something happens to your vessel." },
    // Cider — trickster, costs 1–3
    CardDef { id: "cider-01", deck: Deck::Cider, kind: CardKind::Curse, cost: 1, targets: "one", keywords: NONE,
              title: "Sticky",                     /* 6  -> lg */ text: "Placeholder. Something inconvenient, later." },
    CardDef { id: "cider-02", deck: Deck::Cider, kind: CardKind::Util, cost: 2, targets: "all", keywords: NONE,
              title: "Shuffle",                    /* 7  -> lg */ text: "Placeholder. Everything moves one to the left." },
    CardDef { id: "cider-03", deck: Deck::Cider, kind: CardKind::Reaction, cost: 2, targets: "one", keywords: KW3,
              title: "Not So Fast, Friend",        /* 19 -> md */ text: "Placeholder. A reaction, once reactions exist." },
    CardDef { id: "cider-04", deck: Deck::Cider, kind: CardKind::Atk, cost: 3, targets: "one", keywords: KW6,
              title: "Windfall",                   /* 8  -> lg */ text: "Placeholder. A real hit, for a real price." },
    // Wine — control, costs 2–3
    CardDef { id: "wine-01", deck: Deck::Wine, kind: CardKind::Curse, cost: 2, targets: "one", keywords: NONE,
              title: "Decant",                     /* 6  -> lg */ text: LONG_BODY },
    CardDef { id: "wine-02", deck: Deck::Wine, kind: CardKind::Util, cost: 2, targets: "all", keywords: NONE,
              title: "House Rules Amendment",      /* 21 -> md */ text: "Placeholder. The table agrees to something." },
    CardDef { id: "wine-03", deck: Deck::Wine, kind: CardKind::Buff, cost: 3, targets: "self", keywords: NONE,
              title: "Vintage",                    /* 7  -> lg */ text: "Placeholder. You are briefly untouchable." },
    CardDef { id: "wine-04", deck: Deck::Wine, kind: CardKind::Atk, cost: 3, targets: "one", keywords: NONE,
              title: "Corked",                     /* 6  -> lg */ text: "Placeholder. Control, delivered as damage." },
    // Liquor — burst, costs 2–3
    CardDef { id: "liquor-01", deck: Deck::Liquor, kind: CardKind::Atk, cost: 2, targets: "one", keywords: NONE,
              title: "Shot Called",                /* 11 -> lg */ text: "Placeholder. Loud and immediate." },
    CardDef { id: "liquor-02", deck: Deck::Liquor, kind: CardKind::Atk, cost: 3, targets: "one", keywords: NONE,
              title: "Double",                     /* 6  -> lg */ text: "Placeholder. Louder and more immediate." },
    CardDef { id: "liquor-03", deck: Deck::Liquor, kind: CardKind::Curse, cost: 2, targets: "one", keywords: NONE,
              title: "Hangover",                   /* 8  -> lg */ text: "Placeholder. Payable next round." },
    CardDef { id: "liquor-04", deck: Deck::Liquor, kind: CardKind::Util, cost: 3, targets: "self", keywords: NONE,
              title: "Neat, No Ice, No Mercy",     /* 22 -> md */ text: "Placeholder. Fewer pulls, more effect." },
    // Soft — support, costs 1–2
    CardDef { id: "soft-01", deck: Deck::Soft, kind: CardKind::Buff, cost: 1, targets: "one", keywords: NONE,
              title: "Water Round",                /* 11 -> lg */ text: "Placeholder. Someone feels better." },
    CardDef { id: "soft-02", deck: Deck::Soft, kind: CardKind::Util, cost: 1, targets: "one", keywords: NONE,
              title: "Designated",                 /* 10 -> lg */ text: "Placeholder. You take it for them." },
    CardDef { id: "soft-03", deck: Deck::Soft, kind: CardKind::Buff, cost: 2, targets: "all", keywords: NONE,
              title: "Snack Table",                /* 11 -> lg */ text: "Placeholder. Everyone feels better." },
    CardDef { id: "soft-04", deck: Deck::Soft, kind: CardKind::Reaction, cost: 2, targets: "self", keywords: NONE,
              title: "The Long Sober Look Across The Table",  /* 36 -> sm */ text: "Placeholder. A reaction, once reactions exist." },
];
```

`deck_cards(deck)` returns the four `Card`s for that deck in catalog order;
`card_by_id(id)` scans `CATALOG`. Both map `CardDef` to `Card` field for field,
`keywords` via `.iter().map(|s| s.to_string()).collect()`, `duration: None`.

- [ ] **Step 6: `preview_state()` — the shared runtime fixture builder**

Spec §8 makes this **one builder used by both the tests and the preview route**,
and explicitly **not** `#[cfg(test)]`: Plan A-vis renders the same fixtures at
runtime, and a test-only copy would drift from what the style guide displays.
*"One builder, used by both, so a test failure and a visual regression cannot
disagree about what the fixture is."*

It lives here, in `last_call.rs`, because Task 3's plaque tests already need it —
a full eight seats, two-deck plaques, oversized hands, every title band are all
cases a plain setup form cannot reach by hand. Plan A-vis consumes it and defines
nothing new.

Eight seats, exercising every variant any later plan must display:

```rust
pub fn preview_state() -> LastCallState {
    let mut st = LastCallState::new(vec![
        (1, "alice".into()), (2, "bob".into()),   (3, "cara".into()),
        (4, "dev".into()),   (5, "erin".into()),  (6, "fin".into()),
        (7, "gus".into()), (8, "hal".into()),
    ], 0xC0FFEE);
    st.round = 6;
    st.beat = Beat::Lock;
    st.set_vessel(1, Deck::Beer,   "50cl can").unwrap();
    st.set_vessel(2, Deck::Cider,  "50cl bottle").unwrap();
    st.set_vessel(3, Deck::Wine,   "15cl glass").unwrap();
    st.set_vessel(4, Deck::Liquor, "4cl shot").unwrap();
    st.set_vessel(5, Deck::Soft,   "any").unwrap();
    // two-deck player — README: normal, not an edge case
    st.set_vessel(6, Deck::Beer,   "50cl can").unwrap();
    st.set_vessel(6, Deck::Liquor, "4cl shot").unwrap();
    st.players[2].locked = true;                   // cara: locked
    st.players[4].drawing = true;                  // erin: drawing
    st.players[6].status = Status::Eliminated;     // gus: eliminated
    st.players[6].hp = 0;
    st.players[3].hp = 4;                          // dev: low HP
    // 12 cards: four distinct Cider ids repeated three times. Deliberate —
    // it bypasses set_vessel's dedupe so the n > 8 hand-strip split has a
    // hand to split, and the strip only ever reads a COUNT. Slice 2's
    // HandWheel indexes by card id, so vary the ids before building it.
    // NOTE: not `.repeat(3)` — `[T]::repeat` requires `T: Copy`, and `Card`
    // owns String/Vec<String> fields, so that form does not compile.
    st.players[1].hand = std::iter::repeat_n(lc_cards::deck_cards(Deck::Cider), 3)
        .flatten()
        .collect();
    st.players[0].draws_this_round = 3;            // the plaque's draw badge
    st.set_vessel(8, Deck::Soft, "any").unwrap();  // 8th seat: MAX_SEATS ceiling
    st.deck_counts = vec![
        (Deck::Beer, 21), (Deck::Cider, 17), (Deck::Wine, 4),
        (Deck::Liquor, 0), (Deck::Soft, 12),
    ];
    st.discards = lc_cards::deck_cards(Deck::Beer);   // discard count 4
    st.seq = 42;
    st
}
```

Wine at 4 is the `data-low` amber count; Liquor at 0 is `data-empty` /
RESHUFFLE; bob's 12 cards are the `n > 8` split; fin is the two-deck plaque;
cara is locked; erin is drawing; gus is eliminated. Every assertion in Task 3 and
every swatch in Plan A-vis is a real builder called on real state — nothing is
faked (spec §4.1).

**Eight seats, not seven.** `MAX_SEATS` is 8 and §7.8.1's anchor set runs to
`plaque-seat-7`, so a seven-seat fixture would leave one anchor unprovable when
Plan A-vis asserts them all. Eight also exercises README's *"seven seats is the
ceiling for one ring; eight compresses the two bottom positions"* — which is Plan
B's layout problem, and Plan B will want a fixture that already has it.

Test `test_preview_state_covers_every_variant`:
`players.len() == 8`, all five decks appear, exactly one player has two vessels,
exactly one is `Eliminated`, exactly one is `locked`, exactly one is `drawing`,
one hand has `len() > 8`, `draws_this_round > 0` for exactly one, and
`deck_counts` contains both a `0` and a value in `1..5`. If a later slice edits
the fixture and drops a variant, every downstream swatch silently stops showing
it — this test makes that loud.

- [ ] **Step 7: Tests in `last_call.rs` — these are the spec**

```rust
fn seated() -> LastCallState {
    LastCallState::new(vec![(1, "alice".into()), (2, "bob".into()), (3, "cara".into())], 42)
}
```

1. `test_pull_table` — `Deck::Beer.pulls() == 8`, `Cider == 10`, `Wine == 6`,
   `Liquor == 4`, `Soft == 6`. And after
   `st.set_vessel(1, Deck::Liquor, "4cl shot glass")`, the seat-0 vessel is
   `Vessel { deck: Liquor, pulls_max: 4, pulls_left: 4, container: "4cl shot glass" }`
   — asserting the container string did **not** change `pulls_max`.
2. `test_pull_cost_rounds_up` — exact table:

   | cost | handicap_pct | expected |
   | --- | --- | --- |
   | 2 | 100 | 2 |
   | 3 | 100 | 3 |
   | 1 | 150 | 2 |
   | 2 | 150 | 3 |
   | 3 | 150 | 5 |
   | 3 | 50 | 2 |
   | 2 | 75 | 2 |
   | 1 | 25 | 1 |
   | 4 | 300 | 12 |

3. `test_set_handicap_range` — `st.set_handicap(2, 150)` is `Ok` and stores
   `150`; `st.set_handicap(2, 24)` and `st.set_handicap(2, 301)` are
   `Err(LcError::BadHandicap)` and leave the stored value at `150`;
   `st.set_handicap(999, 150)` is `Err(LcError::NotSeated)`.
4. `test_serde_round_trip` — a state with all three players holding vessels and
   hands, one `Play`, one `Effect`, one card in `discards`, `beat = Beat::Lock`,
   `seq = 7`; assert `LastCallState::from_json(&st.to_json()) == st`.
5. `test_public_view_drops_unrevealed_identity` — the §8 projection test. Give
   each player a different deck, push a `Play` whose `card.title` is `"Corked"`,
   set `beat = Beat::Lock`, then:
   - `view.seats.len() == 3`, `view.seats[0].hand_len == 4`
   - `view.revealed.is_empty()`
   - `serde_json::to_string(&view).unwrap()` contains **none of** `"Corked"`,
     `"beer-01"`, `"cider-01"`, `"wine-01"`.
   Then `beat = Beat::Reveal`: `view.revealed.len() == 1`, the serialized view
   now **does** contain `"Corked"`, and the hand card ids are still absent.
6. `test_public_view_multi_deck_vessels` — a player with Beer *and* Wine
   projects `vessels.len() == 2` with `[(Beer, 8, 8), (Wine, 6, 6)]`,
   `decks() == [Deck::Beer, Deck::Wine]`, and `hand_len == 8`.
7. `test_public_view_carries_plaque_state` — set `players[1].locked = true` and
   `players[2].drawing = true`; the projection carries both through. This is
   spec §3.4's "carries *enough*" half — a projection that dropped them would
   make the plaque unrenderable and Task 3 would not compile.
8. `test_add_player_is_idempotent` — `add_player(2, "bob")` leaves
   `players.len() == 3`; `add_player(9, "dan")` appends at `seat == 3`.
9. `test_beat_order_hues_and_slugs` — `Beat::ORDER` is
   `[Draw, Deal, Diplomacy, Lock, Reveal, Resolve]`; `Draw.index() == 1`,
   `Resolve.index() == 6`, `Resolve.next() == Draw`; hues in `ORDER` are
   `["amber","amber","mint","violet","azure","rose"]`; slugs are
   `["draw","deal","diplomacy","lock","reveal","resolve"]`.
10. `test_stubs_are_not_implemented` — `arm`, `lock_in`, `advance_beat`,
    `resolve` each return `Err(LcError::NotImplemented)` and leave `to_json()`
    unchanged.

- [ ] **Step 8: Tests in `lc_cards.rs` — the catalog is adversarial or it is useless**

1. `test_catalog_costs_match_deck_spread` — every card's cost is inside its
   deck's spread (Beer 1–2, Cider 1–3, Wine 2–3, Liquor 2–3, Soft 1–2), every
   id is unique, and `deck_cards(d).len() == 4` for all five decks.
2. `test_catalog_covers_every_title_band` — count titles by `chars()`:
   **at least one** with `<= 14`, at least one in `15..=24`, and at least one
   `> 24`. Assert the specific expected counts so a later edit that flattens the
   catalog fails loudly: `<= 14` → **15**, `15..=24` → **3**
   (`"Not So Fast, Friend"` 19, `"House Rules Amendment"` 21,
   `"Neat, No Ice, No Mercy"` 22), `> 24` → **2**
   (`"Top Up, Then Top Up Again"` 25, `"The Long Sober Look Across The Table"`
   36).
3. `test_catalog_has_an_overflowing_body` — exactly one card's `text` exceeds
   108 `chars()`, and it is `wine-01`.
4. `test_catalog_has_zero_three_and_six_keyword_cards` — at least one card with
   `keywords.is_empty()`, at least one with exactly 3, and exactly one with 6
   (`cider-04`).
5. `test_catalog_titles_use_char_counts_not_bytes` — every title is ASCII today,
   so assert `title.len() == title.chars().count()` for all 20. When a
   non-ASCII title arrives this test is the reminder that the ramp counts
   `chars()`.

- [ ] **Step 9: Commit**

```bash
git add drinkinggame/src/last_call.rs drinkinggame/src/lc_cards.rs drinkinggame/src/lib.rs
git commit -m "feat(drinks): Last Call object model, PublicView projection and adversarial catalog"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

### Task 2: `assets/lastcall.css` — tokens, scene primitives and every component class

**Class:** A (compiler/lint-gated)

**Why this class:** a stylesheet plus one `include_str!` route. The route is
compile-gated (a missing file is a build error), and the two things CSS can get
silently wrong here — a nested `/*` and a missing asset — are both decided by
the tests added in this task, which run inside `./scripts/verify.sh`.

**Files:**
- Create: `drinkinggame/assets/lastcall.css`
- Modify: `drinkinggame/src/routes.rs` (add `lastcall_css` next to `game_css`,
  register `/assets/lastcall.css` next to `/assets/game.css`)
- Test: `drinkinggame/tests/http.rs`

**Interfaces:**
- Consumes: `Deck::slug()` and `Beat::hue()` from Task 1 — the class names
  `lc_render.rs` emits are `lc-deck-{slug}` and `lc-beat-{hue}`.
- Produces: the complete class contract Tasks 3–6 render against.
  - deck ramps `.lc-deck-{beer,cider,wine,liquor,soft}` binding `--lc-fill`,
    `--lc-ink`, `--lc-on-fill`, `--lc-grid` and `--lc-ink-{59,66,80,99}`
  - beat hues `.lc-beat-{amber,mint,violet,azure,rose}`
  - §7.8 component roots `.lc-cardface`, `.lc-pip`, `.lc-mini`, `.lc-back` +
    `.lc-back-{strip,flight,pile,stack}`, `.lc-dot`, `.lc-plaque`,
    `.lc-handstrip`, `.lc-deckstack`, `.lc-discard`, `#lc-banner`,
    `#lc-beat-timer`, `#lc-felt`, `#lc-flights`, `#lc-hand`
  - §7.5 text classes `.lc-title-{lg,md,sm}`, `.lc-kw`, `.lc-kw-more`,
    `.lc-cardface-expanded`
  - plaque state classes `.is-locked`, `.is-drawing`, `.is-hit`,
    `.is-eliminated`, and stack states via `[data-low]` / `[data-empty]`
  - shell classes `.lc-status`, `.lc-tabs`, `.lc-tab`, `.lc-view`, `.lc-pane`,
    `.lc-actions`, `.lc-btn`, `.lc-btn-secondary`, `.lc-setup`
  - Route: `GET /assets/lastcall.css` → `text/css`

- [ ] **Step 1: Tokens, verbatim**

Open with a section comment in `game.css`'s voice: *"Last Call — its own sheet,
not an extension of game.css: the shell is its own page with its own `<link>`,
game.css is already 832 lines, and one nested `/*` once silently dropped
`.card-big`. Smaller sheets, not one larger one."*

Fonts: copy the nine `@font-face` lines from `game.css:38-46` **verbatim**. A
Last Call page links only this sheet, so it needs its own declarations, and the
relative `url("fonts/…")` paths resolve because `/assets/lastcall.css` is served
from the same URL directory as `/assets/fonts/{name}`. Do not rewrite the paths.

```css
/* tokens */
:root {
  color-scheme: dark;
  --lc-page: #0B0910;
  --lc-device: #0E0C14;
  --lc-panel: #16121F;
  --lc-panel-alt: #17141F;
  --lc-raised: #251F35;
  --lc-focused: #2E2742;
  --lc-back: #1B1628;
  --lc-rail: #2A2340;
  --lc-text: #F2EEF8;
  --lc-body: #CDC6DD;
  --lc-secondary: #A79FBB;
  --lc-label: #8D87A0;
  --lc-faint: #6A6480;
  --lc-mint: #4FD6A8;
  --lc-amber: #FFB570;
  --lc-rose: #F7768E;
  --lc-azure: #6FB6FF;
  --lc-violet: #B48EF7;
  --lc-hair: rgba(242, 238, 248, .10);
  --lc-hair-strong: rgba(242, 238, 248, .22);
  --lc-hair-max: rgba(242, 238, 248, .28);
  --lc-lift-sm: 0 3px 0 rgba(5, 3, 10, .5), 0 8px 16px rgba(5, 3, 10, .42);
  --lc-lift-lg: 0 6px 0 rgba(5, 3, 10, .6), 0 22px 40px rgba(5, 3, 10, .55);
  --lc-ease: cubic-bezier(.2, .8, .3, 1);
  --font-display: 'Archivo', system-ui, sans-serif;
  --font-ui: 'Space Grotesk', system-ui, sans-serif;
  --font-mono: ui-monospace, 'IBM Plex Mono', SFMono-Regular, Menlo, monospace;
}

/* deck ramps — the only taxonomy. fill for solid areas, ink on the dark
   ground; they differ for Wine only, which is too dark to read as text on
   near-black. Renderers emit these class names and never a hex value.
   The four --lc-ink-NN entries are §7.6's deck-tinted border alphas: 59
   subtle, 66 plaque, 80/99 card back. Written out per deck rather than
   composed with color-mix() so they resolve everywhere, and so a reader can
   see which hue each alpha applies to — Wine's is the INK (#D4657F), not the
   fill, which is the one place a copy-paste goes wrong. Soft's --lc-on-fill
   is #0D1620, not #14101D, for the same reason. */
.lc-deck-beer {
  --lc-fill: #FFB570; --lc-ink: #FFB570; --lc-on-fill: #14101D; --lc-grid: #FFB5701a;
  --lc-ink-59: #FFB57059; --lc-ink-66: #FFB57066; --lc-ink-80: #FFB57080; --lc-ink-99: #FFB57099;
}
.lc-deck-cider {
  --lc-fill: #B48EF7; --lc-ink: #B48EF7; --lc-on-fill: #14101D; --lc-grid: #B48EF71a;
  --lc-ink-59: #B48EF759; --lc-ink-66: #B48EF766; --lc-ink-80: #B48EF780; --lc-ink-99: #B48EF799;
}
.lc-deck-wine {
  --lc-fill: #8B2F4A; --lc-ink: #D4657F; --lc-on-fill: #F2EEF8; --lc-grid: #8B2F4A1a;
  --lc-ink-59: #D4657F59; --lc-ink-66: #D4657F66; --lc-ink-80: #D4657F80; --lc-ink-99: #D4657F99;
}
.lc-deck-liquor {
  --lc-fill: #F7768E; --lc-ink: #F7768E; --lc-on-fill: #14101D; --lc-grid: #F7768E1a;
  --lc-ink-59: #F7768E59; --lc-ink-66: #F7768E66; --lc-ink-80: #F7768E80; --lc-ink-99: #F7768E99;
}
.lc-deck-soft {
  --lc-fill: #6FB6FF; --lc-ink: #6FB6FF; --lc-on-fill: #0D1620; --lc-grid: #6FB6FF1a;
  --lc-ink-59: #6FB6FF59; --lc-ink-66: #6FB6FF66; --lc-ink-80: #6FB6FF80; --lc-ink-99: #6FB6FF99;
}

/* beat hues — Deal has no hue in the bundle and inherits Draw's amber */
.lc-beat-amber  { --lc-beat: #FFB570; }
.lc-beat-mint   { --lc-beat: #4FD6A8; }
.lc-beat-violet { --lc-beat: #B48EF7; }
.lc-beat-azure  { --lc-beat: #6FB6FF; }
.lc-beat-rose   { --lc-beat: #F7768E; }
```

- [ ] **Step 2: The base reset — every size in §7.1 depends on it**

`lastcall.css` is standalone: a Last Call page links **only** this file, so it
inherits nothing from `game.css`. This is not boilerplate — without it every
authored size in §7.1 is wrong:

- No `box-sizing: border-box` → `.lc-cardface { height: 176px; padding: 16px 18px }`
  renders **208px** tall, `.lc-mini { width: 62px; padding: 7px 5px }` renders
  72px wide, `.lc-back[data-size="strip"] { width: 16px; height: 24px; padding: 4px }`
  renders 24×32. Nothing in the browser checkpoints measures, so this ships.
- No appearance reset → with `color-scheme: dark` declared in `:root`, the OS
  paints native controls over `.lc-setup select` / `input` / `button`.
  `game.css` carries an explicit comment about exactly this bug.

```css
/* base */
* { box-sizing: border-box; }
html, body { margin: 0; padding: 0; }
p, h1, h2, h3 { margin: 0; }
ul, ol { margin: 0; padding: 0; list-style: none; }
/* Reset native widget painting so author background/color always win —
   without this, OS/browser dark-mode theming (color-scheme: dark) paints
   buttons and inputs with a native dark control instead of our palette. */
button, input, select, textarea { font-family: inherit; color: inherit;
                                  -webkit-appearance: none; appearance: none; }
button { cursor: pointer; }
button:active { transform: scale(.97); }
```

- [ ] **Step 3: Scene primitives (spec §7.6) — including the felt**

The grounds and surfaces, separated from anything that positions players on
them. The felt ships here as a **background primitive**; D.2's angle layout that
puts seats on it is Plan B. The scene is a visual primitive; the seating is
state-driven layout.

```css
/* scene primitives — grounds, panels, hairlines */
.lc-ground     { background: var(--lc-page); }
.lc-device     { background: var(--lc-device); }
.lc-panel      { background: var(--lc-panel); border-radius: 8px; }
.lc-panel-alt  { background: var(--lc-panel-alt); border-radius: 6px; }
.lc-raised     { background: var(--lc-raised); }
.lc-focused    { background: var(--lc-focused); }
.lc-hairline   { border: 1px solid var(--lc-hair); }
.lc-hairline-strong { border: 1px solid var(--lc-hair-strong); }
/* deck-tinted borders — §7.6's alpha ladder, bound in the ramp blocks above */
.lc-edge-subtle { border: 1px solid var(--lc-ink-59); }
.lc-edge-plaque { border: 1px solid var(--lc-ink-66); }
.lc-edge-back   { border: 1px solid var(--lc-ink-80); }

/* the felt — a background primitive. Seat positioning is Plan B. */
#lc-felt {
  position: relative;
  border-radius: 280px;
  border: 11px solid var(--lc-rail);
  background: radial-gradient(ellipse at 50% 44%, #272038 0%, #191430 52%, #100C1B 100%);
  box-shadow:
    inset 0 0 0 2px rgba(242, 238, 248, .14),
    inset 0 50px 110px rgba(5, 3, 10, .5),
    0 26px 80px rgba(5, 3, 10, .75);
}
/* the second hairline ellipse, inset a further 56px */
#lc-felt::after {
  content: ""; position: absolute; inset: 56px;
  border-radius: 280px;
  border: 1px solid var(--lc-hair);
  pointer-events: none;
}
/* the flight layer sits above the felt and eats no pointer events */
#lc-flights { position: absolute; inset: 0; pointer-events: none; overflow: hidden; }
```

The radial gradient, the 11px `#2A2340` rail, the 56px inner ellipse and the
three-part shadow stack are verbatim from the design README — do not simplify
the shadow to a single blur, and do not drop the `inset 0 0 0 2px` line, which
is what gives the felt its edge against the rail.

- [ ] **Step 4: The five card primitives (spec §7.1, module spec B)**

Note the §7.8 roots: **`.lc-cardface`**, not `.lc-card-face`; and `CardBack`
selects its size from `[data-size]`, not from a modifier class, because the
contract makes `data-size` a required attribute.

```css
/* card primitives */
.lc-cardface {
  position: relative; height: 176px; padding: 16px 18px; border-radius: 14px;
  background: var(--lc-raised); border: 1px solid var(--lc-hair-strong);
  box-shadow: var(--lc-lift-sm); display: flex; flex-direction: column; gap: 8px;
  transition: background 190ms var(--lc-ease), border-color 190ms var(--lc-ease);
}
.lc-cardface .lc-face-top { display: flex; align-items: center; justify-content: space-between; }
.lc-cardface .lc-face-deck {
  font-family: var(--font-ui); font-weight: 700; font-size: 10px;
  letter-spacing: .13em; text-transform: uppercase; color: var(--lc-ink);
}
.lc-cardface .lc-face-title {
  font-family: var(--font-display); font-weight: 900; letter-spacing: -.03em;
  color: var(--lc-text); line-height: 1.02;
}
.lc-cardface .lc-face-body {
  font-family: var(--font-ui); font-weight: 400; font-size: 15px;
  line-height: 1.35; color: var(--lc-body);
}
.lc-kw {
  display: inline-block; padding: 2px 8px; border-radius: 999px;
  font-family: var(--font-ui); font-weight: 700; font-size: 9.5px;
  letter-spacing: .12em; text-transform: uppercase;
  color: var(--lc-ink); border: 1px solid var(--lc-hair);
}

.lc-pip {
  display: inline-block; padding: 2px 11px; border-radius: 5px;
  background: var(--lc-fill); color: var(--lc-on-fill);
  font-family: var(--font-display); font-weight: 900; font-size: 17px;
}

.lc-mini {
  width: 62px; padding: 7px 5px; border-radius: 6px;
  background: var(--lc-panel-alt); border: 1.5px solid var(--lc-ink);
  box-shadow: var(--lc-lift-sm); text-align: left;
}
.lc-mini .lc-mini-cost {
  font-family: var(--font-display); font-weight: 800; font-size: 9px; color: var(--lc-ink);
}
.lc-mini .lc-mini-title {
  font-family: var(--font-display); font-weight: 800; font-size: 10px; line-height: 1.1;
  color: var(--lc-text); display: -webkit-box; -webkit-line-clamp: 2;
  -webkit-box-orient: vertical; overflow: hidden;
}

/* The grid IS the card back — keep it at every size. */
.lc-back {
  background-color: var(--lc-back); border: 1px solid var(--lc-ink-80);
  border-radius: 3px;
  background-image:
    linear-gradient(var(--lc-grid) 1px, transparent 1px),
    linear-gradient(90deg, var(--lc-grid) 1px, transparent 1px);
  background-size: 10px 10px;
  background-origin: content-box; padding: 5px;
}
.lc-back[data-size="strip"]  { width: 16px; height: 24px; background-size: 9px 9px; padding: 4px; }
.lc-back[data-size="flight"] { width: 44px; height: 62px; background-size: 9px 9px; padding: 4px; }
.lc-back[data-size="pile"]   { width: 46px; height: 62px; }
.lc-back[data-size="stack"]  { width: 68px; height: 92px; }

.lc-dot {
  width: 8px; height: 8px; border-radius: 50%;
  background: var(--lc-fill); box-shadow: 0 0 8px var(--lc-fill);
}
```

- [ ] **Step 5: Text handling (spec §7.5)**

The bundle has **no** `line-clamp` and **no** `text-overflow` anywhere: every
prototype card has short text, so overflow behaviour was never rendered. These
rules are designed, not transcribed; Task 3 picks the ramp class server-side.

```css
/* text handling — CardFace is fluid x 176px FIXED, so text cannot grow */
.lc-title-lg { font-size: 30px; }   /* title <= 14 chars — the authored size */
.lc-title-md { font-size: 24px; }   /* 15-24 chars */
.lc-title-sm { font-size: 20px; }   /* > 24 chars */

.lc-cardface .lc-face-title {
  display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical;
  overflow: hidden;
}
.lc-cardface .lc-face-body {
  display: -webkit-box; -webkit-line-clamp: 3; -webkit-box-orient: vertical;
  overflow: hidden;
}
.lc-kw-more { border-style: dashed; }

/* the expanded variant — a detail view. This slice ships the variant and the
   data-expandable marking; which gesture opens it is the hand-group slice. */
.lc-cardface-expanded { height: auto; min-height: 176px; }
.lc-cardface-expanded .lc-face-title,
.lc-cardface-expanded .lc-face-body {
  display: block; -webkit-line-clamp: none; overflow: visible;
}
```

`-webkit-line-clamp` with `display: -webkit-box` is the only clamp that works in
every browser this app sees. Keep both the `display` and the
`-webkit-box-orient` lines — the clamp silently does nothing without them, which
is exactly the kind of failure that ships.

- [ ] **Step 6: The table components — plaque, hand strip, deck stack, discard**

§7.6's component/positioning split puts all four here. Their **placement** on
the felt ellipse is Plan B; their anatomy is not.

```css
/* PlayerPlaque (D.1) — 204px, three stacked rows. Plan B positions it. */
.lc-plaque {
  width: 204px; padding: 11px 14px 12px; border-radius: 10px;
  background: var(--lc-panel); border: 1px solid var(--lc-ink-66);
  box-shadow: var(--lc-lift-sm);
  display: flex; flex-direction: column; gap: 9px;
}
.lc-plaque .lc-identity { display: flex; align-items: baseline; justify-content: space-between; }
.lc-plaque .lc-name { font-family: var(--font-display); font-weight: 900; font-size: 22px;
                      letter-spacing: -.025em; color: var(--lc-text); }
.lc-plaque .lc-hp   { font-family: var(--font-display); font-weight: 900; font-size: 28px;
                      letter-spacing: -.03em; color: var(--lc-text); }
.lc-plaque .lc-drinks { display: flex; align-items: center; gap: 3px; }
.lc-plaque .lc-decknames { font-family: var(--font-mono); font-size: 11px; color: var(--lc-label); }
.lc-plaque .lc-draws { margin-left: auto; border-radius: 4px; padding: 1px 6px;
                       background: var(--lc-ink-59);
                       font-family: var(--font-display); font-weight: 900; font-size: 13px;
                       color: var(--lc-text); }
.lc-plaque .lc-handstrip { border-top: 1px solid var(--lc-hair); padding-top: 9px; }

/* the 3px top rule — one deck fills it, two split it 50/50 */
.lc-rule { height: 3px; border-radius: 3px 3px 0 0; overflow: hidden; display: flex; }
.lc-rule-1 { background: var(--lc-fill, var(--lc-hair)); }
.lc-rule-2 i { flex: 1; background: var(--lc-fill); }

/* HandStrip (D.3) — overlapping backs, exact count right-aligned */
.lc-handstrip { display: flex; align-items: center; gap: 0; }
.lc-handstrip .lc-back + .lc-back { margin-left: -4px; }
.lc-handstrip-more  { font-family: var(--font-display); font-weight: 900; font-size: 13px;
                      color: var(--lc-body); margin-left: 5px; }
.lc-handstrip-count { margin-left: auto; font-family: var(--font-mono); font-size: 10px;
                      color: var(--lc-label); }

/* DeckStack + DiscardSlot (D.4) */
.lc-deckstack, .lc-discard {
  position: relative; display: inline-flex; flex-direction: column;
  align-items: center; gap: 6px;
}
/* the offset shadow card behind the stack, at +3px */
.lc-deckstack .lc-back::before {
  content: ""; position: absolute; inset: 0; transform: translate(3px, 3px);
  border-radius: 3px; background: var(--lc-back); z-index: -1;
}
.lc-deckstack-count { font-family: var(--font-display); font-weight: 900; font-size: 22px;
                      color: var(--lc-ink); }
.lc-deckstack-name  { font-family: var(--font-ui); font-weight: 700; font-size: 9px;
                      letter-spacing: .13em; text-transform: uppercase; color: var(--lc-label); }
/* a destination, not a deck: dashed, no grid, neutral count */
.lc-discard .lc-back { background-image: none; border-style: dashed; }
.lc-discard .lc-deckstack-count { color: var(--lc-body); }

/* deck-stack states, driven by the contract's exposed attributes */
.lc-deckstack[data-low]   .lc-deckstack-count { color: var(--lc-amber); }
.lc-deckstack[data-empty] .lc-deckstack-count { font-size: 11px; letter-spacing: .1em;
                                                color: var(--lc-amber); }

/* plaque states — idle is the base. locked/drawing/hit/eliminated layer on.
   The animations for is-hit and is-drawing are authored in Plan A-vis. */
.lc-lock-tick { color: var(--lc-violet); font-size: 13px; margin-left: 6px; }
.lc-plaque:not(.is-locked) .lc-lock-tick { display: none; }
.lc-plaque.is-eliminated { opacity: .4; }
.lc-plaque.is-eliminated .lc-hp { font-size: 15px; letter-spacing: .1em; color: var(--lc-label); }
```

- [ ] **Step 7: The F.1 phone shell (spec §7.3)**

Fixed vertical order, no screen may reorder it: status row → phase banner → tab
row → view → action bar. The view flexes to fill; the action bar is the thumb
zone.

```css
/* F.1 phone shell */
body.lc { margin: 0; min-height: 100vh; display: flex; flex-direction: column;
          background: var(--lc-device); color: var(--lc-text); font-family: var(--font-ui); }
.lc-status { height: 40px; padding: 0 20px; display: flex; align-items: center;
             justify-content: space-between; font-family: var(--font-mono);
             font-size: 12px; color: var(--lc-label); }
#lc-banner { padding: 0 18px 12px; display: flex; align-items: baseline;
             justify-content: space-between; gap: 12px; }
.lc-banner-beat { font-family: var(--font-display); font-weight: 900; font-size: 26px;
                  letter-spacing: -.02em; text-transform: uppercase;
                  color: var(--lc-beat, var(--lc-text)); }
.lc-banner-meta { font-family: var(--font-ui); font-weight: 700; font-size: 10px;
                  letter-spacing: .13em; text-transform: uppercase; color: var(--lc-label); }
.lc-tabs { padding: 0 18px; display: flex; border-bottom: 1px solid var(--lc-hair); }
.lc-tab { background: none; border: 0; padding: 9px 14px 8px; cursor: pointer;
          font-family: var(--font-display); font-weight: 800; font-size: 11px;
          letter-spacing: .1em; color: var(--lc-label);
          border-bottom: 2px solid transparent; }
.lc-tab[aria-selected="true"] { color: var(--lc-text); }
.lc-tab[data-lc-tab="hand"][aria-selected="true"]  { border-bottom-color: #B48EF7; }
.lc-tab[data-lc-tab="table"][aria-selected="true"] { border-bottom-color: #6FB6FF; }
.lc-tab[data-lc-tab="log"][aria-selected="true"]   { border-bottom-color: #8D87A0; }
.lc-view { flex: 1; overflow-y: auto; padding: 14px 12px 0; }
.lc-pane[hidden] { display: none; }
.lc-empty { padding: 40px 8px; text-align: center; font-size: 13px; line-height: 1.5;
            color: var(--lc-faint); }
.lc-actions { padding: 12px 14px 18px; display: flex; gap: 10px; }
.lc-btn { flex: 1; height: 64px; border: 0; border-radius: 8px; cursor: pointer;
          background: var(--lc-fill, var(--lc-amber)); color: #14101D;
          font-family: var(--font-display); font-weight: 900; font-size: 20px;
          letter-spacing: .02em; transition: transform 130ms var(--lc-ease); }
.lc-btn:active { transform: scale(.97); }
.lc-btn-secondary { flex: 0 0 92px; background: transparent; color: var(--lc-body);
                    border: 1px solid var(--lc-hair-strong); }
/* the option that involves drinking is always the amber one */
.lc-btn-drink { background: var(--lc-amber); }

/* hand region — deliberately throwaway. Slice 2's HandWheel replaces this
   container, not the CardFace inside it. */
#lc-hand { display: flex; flex-direction: column; gap: 12px; padding-bottom: 14px; }

/* setup — plain and undesigned on purpose (spec §2, item 2). Plan A2 fills it. */
.lc-setup { background: var(--lc-panel); border-radius: 8px; padding: 14px;
            margin-bottom: 14px; display: flex; flex-direction: column; gap: 10px; }
.lc-setup h2 { font-family: var(--font-display); font-weight: 900; font-size: 15px;
               letter-spacing: .04em; text-transform: uppercase; color: var(--lc-text); }
.lc-setup form { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; }
.lc-setup select, .lc-setup input { background: var(--lc-panel-alt); color: var(--lc-text);
            border: 1px solid var(--lc-hair); border-radius: 6px; padding: 8px 10px;
            font-size: 14px; }
.lc-setup button { background: var(--lc-panel-alt); color: var(--lc-text);
            border: 1px solid var(--lc-hair-strong); border-radius: 6px;
            padding: 8px 14px; font-size: 13px; cursor: pointer; }
.lc-setup-row { display: flex; align-items: center; gap: 8px; font-size: 13px;
                color: var(--lc-body); }
```

Do **not** add a `@media (prefers-reduced-motion: reduce)` block here — **Plan
A-vis** authors the single one alongside the keyframes, and two blocks means the
second silently overrides half of the first.

- [ ] **Step 8: Register the asset route**

In `routes.rs`, directly under `game_css`:

```rust
async fn lastcall_css() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css")],
        include_str!("../assets/lastcall.css"),
    )
}
```

and in `router()`, directly under the `/assets/game.css` line:

```rust
.route("/assets/lastcall.css", get(lastcall_css))
```

- [ ] **Step 9: Extend the asset tests**

In `drinkinggame/tests/http.rs`:

1. Add `("/assets/lastcall.css", "text/css")` to the loop in
   `test_assets_are_served`.
2. Generalise `test_game_css_has_no_nested_comment_markers` — extract its body
   into `async fn assert_no_nested_comments(app: &Router, path: &str)` and call
   it for both `/assets/game.css` and `/assets/lastcall.css`, keeping the doc
   comment and panic message (add `{path}` to the message). Do not duplicate the
   scanner.
3. `test_lastcall_css_has_deck_ramps` — the sheet contains `.lc-deck-beer`,
   `.lc-deck-cider`, `.lc-deck-wine`, `.lc-deck-liquor`, `.lc-deck-soft`, the
   Wine ink `#D4657F` **and** `--lc-ink-66: #D4657F66` (the alpha ladder built
   on the ink, not the fill — the one a copy-paste breaks), and Soft's
   `#0D1620`.
4. `test_lastcall_css_has_base_reset` — contains `box-sizing: border-box` and
   `appearance: none`. Without the first, every authored size is off by its
   padding; without the second the OS paints native controls. Nothing in the
   browser checkpoints measures either, so only this test catches it.
5. `test_lastcall_css_has_every_component_root` — contains every §7.8 root:
   `.lc-cardface`, `.lc-pip`, `.lc-mini`, `.lc-back`, `.lc-dot`, `.lc-plaque`,
   `.lc-handstrip`, `.lc-deckstack`, `.lc-discard`, `#lc-banner`, `#lc-felt`,
   `#lc-flights`, `#lc-hand`. A contract table nobody styles is a contract
   nobody keeps.

- [ ] **Step 10: Commit**

```bash
git add drinkinggame/assets/lastcall.css drinkinggame/src/routes.rs drinkinggame/tests/http.rs
git commit -m "feat(drinks): Last Call design tokens, scene primitives and component classes"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

### Task 3: `lc_render.rs` — every component, to the §7.8 contract

**Class:** B (logic, tests specified below)

**Why this class:** formatted-string builders whose entire contract is "given
this data, emit this markup with these attributes". The Module Spec's own step-1
acceptance line and the whole §7.8 attribute table are written below as tests
with their expected substrings — the tests are the spec.

**Files:**
- Create: `drinkinggame/src/lc_render.rs`
- Modify: `drinkinggame/src/lib.rs` (`pub mod lc_render;`)
- Test: `drinkinggame/src/lc_render.rs` `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: `crate::last_call::{Card, Deck, Beat, Status, PublicView,
  PublicSeat, DECK_LOW_THRESHOLD, preview_state}` (Task 1) — the plaque and
  hand-strip tests below build their `PublicSeat` fixtures from
  `preview_state().public_view()` rather than hand-rolling one, so a test
  failure and Plan A-vis's gallery cannot disagree about what the fixture is;
  `crate::render::html_escape(&str) -> String` (existing, `render.rs:14`);
  the CSS class contract from Task 2.
- Produces:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackSize { Strip, Flight, Pile, Stack }   // 16x24 / 44x62 / 46x62 / 68x92
impl BackSize { pub fn slug(self) -> &'static str; }   // the `data-size` value

// ---- card primitives ----
/// Private — takes the viewer's own card.
pub fn card_face(card: &Card) -> String;
/// The expanded detail variant: height auto, no clamps, no chip cap.
pub fn card_face_expanded(card: &Card) -> String;
pub fn card_pip(card: &Card) -> String;
pub fn card_mini(card: &Card) -> String;
/// Public by construction — take a Deck, never a Card.
pub fn card_back(deck: Deck, size: BackSize) -> String;
pub fn card_dot(deck: Deck) -> String;

// ---- §7.5 text handling. Decided SERVER-SIDE, from the string. ----
pub const TITLE_RAMP_MD_CHARS: usize = 14;   // > this -> .lc-title-md (24px)
pub const TITLE_RAMP_SM_CHARS: usize = 24;   // > this -> .lc-title-sm (20px)
pub const TITLE_CLAMP_CHARS: usize = 44;
pub const BODY_CLAMP_CHARS: usize = 108;
pub const MAX_KEYWORD_CHIPS: usize = 3;
pub fn title_ramp_class(title: &str) -> &'static str;
pub fn is_truncated(card: &Card) -> bool;

// ---- table components (§7.6's component half) ----
/// D.1 PlayerPlaque, all five states. Takes a projected seat, never an
/// `LcPlayer` — the plaque is a public surface (spec §3.4).
pub fn player_plaque(seat: &PublicSeat) -> String;
/// D.3 HandStrip. n <= 8 -> n backs; n > 8 -> 7 backs + "+{n-7}".
pub fn hand_strip(decks: &[Deck], n: usize) -> String;
/// The 3px plaque top rule. One deck fills it; two split it 50/50.
pub fn deck_rule(decks: &[Deck]) -> String;
/// D.4 DeckStack. `data-low` under DECK_LOW_THRESHOLD, `data-empty` at 0
/// (count reads RESHUFFLE).
pub fn deck_stack(deck: Deck, count: u16) -> String;
/// D.4 DiscardSlot — a destination, not a deck.
pub fn discard_slot(count: usize) -> String;

// ---- shell components, from the projection ----
pub fn lc_banner(view: &PublicView) -> String;
pub fn beat_timer(duration_ms: u32, elapsed_ms: u32) -> String;
pub fn lc_public_panel(view: &PublicView) -> String;
```

**Not in this plan:** `lc_hand_pane` and the setup form — the private hand
fragment belongs to Plan A2, which owns the routes it posts to. Plan B adds only
the D.2 ellipse angle layout that positions these plaques.

- [ ] **Step 1: The five card primitives, with their contract attributes**

Follow `render.rs`'s existing approach: `format!` over raw strings, every
player-supplied string through `html_escape`. Module doc comment: *"Last Call
fragments as formatted strings, matching `render.rs`. Public builders take
`&PublicView`/`&PublicSeat` — never `&LastCallState` — so an unrevealed card
cannot reach a broadcast fragment by construction (spec §3.4). Every root and
attribute here is the §7.8 contract; changing one is a breaking change for Plan
A2 and Plan B."*

The §7.8 required attributes are not optional decoration — Plan A2 selects on
them:

```
card_face  -> <article class="lc-cardface lc-deck-{slug}"
                       data-card-id="{id}" data-deck="{slug}" data-cost="{cost}"
                       [data-expandable]>
                <div class="lc-face-top"><span class="lc-face-deck">{DECK}</span>{pip}</div>
                <h3 class="lc-face-title {ramp}">{title}</h3>
                <p class="lc-face-body">{text}</p>
                [<div class="lc-face-kws">{chips}[<span class="lc-kw lc-kw-more">+{n}</span>]</div>]
              </article>
card_pip   -> <span class="lc-pip lc-deck-{slug}" data-deck="{slug}" data-cost="{cost}">{cost}</span>
card_mini  -> <div class="lc-mini lc-deck-{slug}" data-card-id="{id}" data-deck="{slug}" data-cost="{cost}">
                <span class="lc-mini-cost">{cost}</span>
                <span class="lc-mini-title">{title}</span></div>
card_back  -> <div class="lc-back lc-deck-{slug}" data-deck="{slug}" data-size="{strip|flight|pile|stack}"></div>
card_dot   -> <span class="lc-dot lc-deck-{slug}" data-deck="{slug}"></span>
```

`card_face` nests `card_pip(card)` — one cost pip, one implementation.

**No `hx-post`, no `hx-get`, no `onclick`, anywhere.** The contract is structure,
never behaviour: `[data-card-id]` exists and is the click target; what tapping
*does* is slice 2 and 3. If a builder starts describing an interaction, it has
drifted out of scope.

- [ ] **Step 2: The §7.5 text rules — server-side, from the string**

`CardFace` is fluid × **176px fixed**, so text cannot simply grow. The
prototypes never exercise this: `Game UI.dc.html` contains no `line-clamp` and
no `text-overflow` at all. So these rules are designed rather than transcribed,
and the decision is made **server-side from the string** — not by CSS reflow —
so it is deterministic and testable.

```rust
pub fn title_ramp_class(title: &str) -> &'static str {
    match title.chars().count() {
        0..=TITLE_RAMP_MD_CHARS => "lc-title-lg",       // <= 14 -> 30px
        n if n <= TITLE_RAMP_SM_CHARS => "lc-title-md", // 15-24 -> 24px
        _ => "lc-title-sm",                             // > 24  -> 20px
    }
}

pub fn is_truncated(card: &Card) -> bool {
    card.title.chars().count() > TITLE_CLAMP_CHARS
        || card.text.chars().count() > BODY_CLAMP_CHARS
        || card.keywords.len() > MAX_KEYWORD_CHIPS
}
```

`card_face` puts `title_ramp_class(&card.title)` on `.lc-face-title`, renders at
most `MAX_KEYWORD_CHIPS` chips followed by `+{n-3}` when there are more, and
sets `data-expandable` when `is_truncated(card)`.

**The two mechanisms are not the same thing, and the plan says which is
authoritative.** The CSS clamps (Task 2, Step 5) are what actually truncates on
screen. `is_truncated` is the server's *estimate* that the clamp will bite, and
it exists only to decide the `data-expandable` marking. The character thresholds
are therefore deliberately **conservative — they mark early rather than late**:
a card marked expandable that happened to fit costs nothing, while a card that
got clipped and was not marked has silently lost rules text, which is the
failure this section exists to prevent. They are plain `pub const`s so a
playtest can move them without touching logic.

`card_face_expanded(card)` is `card_face(card)` plus `lc-cardface-expanded` and
no chip cap. Implement both over a private
`fn face(card: &Card, expanded: bool) -> String` so the two cannot drift.

- [ ] **Step 3: HandStrip and the deck rule**

**HandStrip** shows hand size without making anyone read a number: overlapping
16×24 CardBacks at −4px margin, **cycling through that player's deck colours**
so a two-deck hand reads as two-deck, with the exact count right-aligned.

```
n <= 8  ->  n backs
n >  8  ->  7 backs + "+{n-7}" in Archivo 900/13px
```

```
<div class="lc-handstrip" data-hand-size="{n}" data-decks="{comma-joined slugs}">
  {card_back(decks[i % decks.len()], BackSize::Strip) for i in 0..shown}
  [<span class="lc-handstrip-more">+{n-7}</span>]
  <span class="lc-handstrip-count">{n}</span>
</div>
```

An empty `decks` slice renders the backs in the Beer ramp rather than panicking
on `% 0` — guard it explicitly; a seat with no registered vessel is reachable in
Plan A2 between joining and registering a drink.

**deck_rule** is the 3px plaque top rule. Two `<i>` halves rather than an inline
gradient, because Task 2's CSS owns colour and a gradient would need hex in the
renderer:

```
<div class="lc-rule lc-rule-1 lc-deck-{slug}"></div>                                    one deck
<div class="lc-rule lc-rule-2"><i class="lc-deck-{a}"></i><i class="lc-deck-{b}"></i></div>   two
```

Three or more decks split into that many equal parts by the same mechanism.
README: *"Four of the seven seats in the prototype run two drinks; this is
normal, not an edge case."*

- [ ] **Step 4: PlayerPlaque (D.1) and its five states**

204px wide, `#16121F`, `1px solid <ink>66`, `border-top` from `deck_rule`, r10,
`padding: 11px 14px 12px`, with the small hard lift. Three stacked rows:
identity (name left, HP right) · drinks (one 8px dot per vessel, then the deck
names joined by `+`, then `seat.draws` right-aligned as a deck-tinted badge —
**omitted entirely when `draws == 0`**, because a badge reading `0` on eight
plaques at once is noise) · HandStrip, separated by a hairline with 9px above
and below.

```
<div class="lc-plaque lc-deck-{first_slug}{ state classes }"
     data-seat="{seat}" data-decks="{comma-joined slugs}" data-hp="{hp}"
     data-status="{alive|eliminated}" data-hand-size="{hand_len}"
     data-flight-anchor="plaque-seat-{seat}">
  {deck_rule(&decks)}
  <div class="lc-identity">
    <span class="lc-name">{name}<span class="lc-lock-tick">&#9679;</span></span>
    <span class="lc-hp">{hp or GHOST}</span>
  </div>
  <div class="lc-drinks">{dots}<span class="lc-decknames">{Beer + Liquor}</span>
       [<span class="lc-draws">{draws}</span>]</div>
  {hand_strip(&decks, hand_len)}
</div>
```

The five states, exactly (idle is the base — no extra class):

| State | Class | Effect |
| --- | --- | --- |
| idle | — | the base plaque |
| locked | `is-locked` | violet tick beside the name |
| drawing | `is-drawing` | the deck rule pulses (Plan A-vis animates) |
| hit | `is-hit` | 4px shake + HP flash to rose (Plan A-vis animates) |
| eliminated | `is-eliminated` | whole plaque to 40%, HP replaced by `GHOST` |

`locked` and `drawing` come from `PublicSeat`; `eliminated` from
`status == Status::Eliminated`. **`is-hit` is not derivable from `PublicSeat`** —
it is a transient event, not a state, so `player_plaque` never emits it; slice 3
adds and removes the class from the client. The preview adds it by hand to
demonstrate the animation. Write that distinction as a comment on the function,
because the obvious "add a `hit: bool` to `PublicSeat`" is wrong: a broadcast
snapshot has no way to say "was hit just now" without leaking timing into state.

**Motion anchor** `data-flight-anchor="plaque-seat-{seat}"` is required on every
plaque even though nothing fires a flight until slice 3 (§7.8.1).

- [ ] **Step 5: DeckStack, DiscardSlot, the banner and the beat timer**

```
deck_stack(deck, count) ->
  <div class="lc-deckstack lc-deck-{slug}" data-deck="{slug}" data-count="{count}"
       [data-low] [data-empty] data-flight-anchor="deck-{slug}">
    {card_back(deck, BackSize::Stack)}
    <span class="lc-deckstack-count">{count or RESHUFFLE}</span>
    <span class="lc-deckstack-name">{DECK}</span>
  </div>
```

`data-low` when `0 < count < DECK_LOW_THRESHOLD` (5); `data-empty` when
`count == 0`, and then the count element reads `RESHUFFLE` instead of `0`.
Emit the attributes as bare presence attributes, not `data-low="false"` — the
CSS selects on `[data-low]`, so a `"false"` value would style every stack.

```
discard_slot(count) ->
  <div class="lc-discard" data-count="{count}" data-flight-anchor="discard">
    <div class="lc-back" data-size="stack"></div>
    <span class="lc-deckstack-count">{count}</span>
    <span class="lc-deckstack-name">DISCARD</span>
  </div>
```

The discard has the same footprint but a dashed hairline, no grid and a neutral
count — it is a destination, not a deck, and it carries no `data-deck`.

```
lc_banner(view) ->
  <div class="lc-banner lc-beat-{hue}" id="lc-banner"
       data-beat="{beat_slug}" data-round="{round}">
    <span class="lc-banner-beat">{BEAT LABEL}</span>
    <span class="lc-banner-meta">ROUND {round} &middot; BEAT {index} OF 6</span>
  </div>

beat_timer(duration_ms, elapsed_ms) ->
  <div id="lc-beat-timer" class="lc-timer" data-duration-ms="{d}" data-elapsed-ms="{e}"
       style="--lc-beat-ms:{remaining}ms"></div>
```

`lc_banner` returns the **whole element**, hue class included — the beat and its
hue are one decision and must not be split across the renderer and a template.
The timer's inline `style` sets a duration custom property only; it carries no
colour, so the no-hex rule holds.

`lc_public_panel(view)` is the payload of the `LcPublic` SSE message (Plan A2).
Its Plan A body is the banner plus the seq marker; **Plan A2 and Plan B extend
the body, never the signature**:

```rust
pub fn lc_public_panel(view: &PublicView) -> String {
    format!(
        r#"<div data-lc-public data-seq="{seq}"><template data-lc-banner>{banner}</template></div>"#,
        seq = view.seq, banner = lc_banner(view),
    )
}
```

The `<template data-lc-banner>` wrapper mirrors the existing `room` event's
`<template data-topbar>` convention in `room.html` — one SSE message carrying
several destinations.

- [ ] **Step 6: Tests — these are the spec**

1. `test_one_card_renders_at_five_sizes_in_five_deck_colours` — the Module
   Spec's step-1 acceptance line. For each `deck` in `Deck::ALL`, with
   `card = &lc_cards::deck_cards(deck)[0]`:
   - `card_face(card)` contains `lc-cardface`, `lc-deck-{slug}`, the title, and
     a nested `class="lc-pip lc-deck-{slug}"`;
   - `card_pip(card)` contains `lc-pip`, `lc-deck-{slug}`, `>{cost}<`;
   - `card_mini(card)` contains `lc-mini`, `lc-deck-{slug}`;
   - `card_back(deck, s)` for all four sizes contains `lc-back`,
     `lc-deck-{slug}` and `data-size="{slug}"`;
   - `card_dot(deck)` contains `lc-dot`, `lc-deck-{slug}`.
   25 renderings in one loop, and **no** output contains a `#` hex colour.
2. `test_contract_attributes_are_present` — the §7.8 table, asserted directly.
   `card_face` has `data-card-id`, `data-deck`, `data-cost`; `card_pip` has
   `data-deck`, `data-cost`; `card_mini` has all three; `card_back` has
   `data-deck` and `data-size`; `card_dot` has `data-deck`; `player_plaque` has
   `data-seat`, `data-decks`, `data-hp`, `data-status`, `data-hand-size`;
   `hand_strip` has `data-hand-size` and `data-decks`; `deck_stack` has
   `data-deck` and `data-count`; `discard_slot` has `data-count`; `lc_banner`
   has `data-beat` and `data-round`; `beat_timer` has `data-duration-ms` and
   `data-elapsed-ms`. Plan A2 selects on these; a rename here is a silent break
   there.
3. `test_no_builder_emits_behaviour` — no output of any builder contains
   `hx-post`, `hx-get`, `hx-swap`, `onclick` or `href`. The contract is
   structure, never behaviour, and this is the only mechanical way to hold that
   line.
4. `test_backs_and_dots_carry_no_card_identity` — for every card in `CATALOG`,
   neither `card_back(card.deck, size)` (all four) nor `card_dot(card.deck)`
   contains the card's `id` or `title`. "A card they do not own is never shown
   as a face", asserted structurally.
5. `test_card_face_escapes_text` — a `Card` with `title: "<script>x</script>"`
   renders `&lt;script&gt;` and never a literal `<script>`.
6. `test_title_ramp_thresholds` — exact table:

   | title | chars | class |
   | --- | --- | --- |
   | `"Neat"` | 4 | `lc-title-lg` |
   | `"Second Wind"` | 11 | `lc-title-lg` |
   | `"Fourteen chars"` | 14 | `lc-title-lg` |
   | `"Fifteen chars!!"` | 15 | `lc-title-md` |
   | `"Twenty-four characters!!"` | 24 | `lc-title-md` |
   | `"Twenty-five characters!!!"` | 25 | `lc-title-sm` |
   | `"The Long Sober Look Across The Table"` | 36 | `lc-title-sm` |

   Assert on `title_ramp_class()` **and** that `card_face` puts the class on
   `.lc-face-title`. Include `"Königsschlucküberraschung"` (25 chars / 27 bytes)
   → `lc-title-sm`, proving the ramp counts `chars()` not `len()`.
7. `test_is_truncated_marks_expandable` — title 44 chars → `false`, 45 → `true`;
   body 108 → `false`, 109 → `true`; 3 keywords → `false`, 4 → `true`; a card
   failing none emits no `data-expandable`, a card failing any one emits it.
   Include the real `wine-01` from the catalog: `is_truncated` is `true` and
   `card_face` marks it — the adversarial catalog's whole purpose.
8. `test_keyword_chips_cap_at_three` — 0 keywords → no `.lc-kw`, no
   `.lc-kw-more`; 3 → three `.lc-kw`, no `.lc-kw-more`; 6 (the real `cider-04`)
   → three `.lc-kw` plus one `.lc-kw-more` containing `+3`, and the three
   rendered are the **first** three in order.
9. `test_card_face_expanded_drops_clamps_and_caps` — for `cider-04` with a long
   body: `card_face_expanded` contains `lc-cardface-expanded`, the full body,
   all six chips, no `lc-kw-more`; `card_face` on the same card contains
   `data-expandable`, `lc-kw-more`, and the **same** ramp class (the ramp is a
   size decision, not a truncation one — it applies in both).
10. `test_hand_strip_split` — the §8 rule, with `decks = &[Deck::Beer]`:

    | n | backs rendered | `+n` chip |
    | --- | --- | --- |
    | 0 | 0 | absent |
    | 1 | 1 | absent |
    | 8 | 8 | absent |
    | 9 | 7 | `+2` |
    | 12 | 7 | `+5` |
    | 30 | 7 | `+23` |

    `data-hand-size="{n}"` and the count `{n}` are present in every case,
    including `n = 0`.
11. `test_hand_strip_cycles_deck_colours` — `hand_strip(&[Beer, Wine], 4)`
    renders backs in the order beer, wine, beer, wine (assert the four
    `lc-deck-*` occurrences in source order) and `data-decks="beer,wine"`;
    `hand_strip(&[], 3)` renders three `lc-deck-beer` backs and does not panic.
12. `test_deck_rule_splits_for_multi_deck` — `deck_rule(&[Wine])` contains
    `lc-rule-1`, `lc-deck-wine`, no `<i`; `deck_rule(&[Beer, Wine])` contains
    `lc-rule-2` and exactly two `<i` halves; `deck_rule(&[])` renders a neutral
    rule rather than panicking. No `#` in any output.
13. `test_deck_stack_states` — exact table:

    | count | `data-low` | `data-empty` | count text |
    | --- | --- | --- | --- |
    | 21 | absent | absent | `21` |
    | 5 | absent | absent | `5` |
    | 4 | present | absent | `4` |
    | 1 | present | absent | `1` |
    | 0 | absent | present | `RESHUFFLE` |

    And the attributes are bare (`data-low>` / `data-low `), never
    `data-low="false"` — a `"false"` value would match `[data-low]` and turn
    every stack amber.
14. `test_plaque_five_states` — from a `PublicSeat` fixture:
    - idle → contains none of `is-locked`, `is-drawing`, `is-hit`,
      `is-eliminated`;
    - `locked = true` → `is-locked` and an `lc-lock-tick`, and **no card ids**
      anywhere in the output (DDv2 §6.3: a lock tick per seat, never the armed
      cards);
    - `drawing = true` → `is-drawing`;
    - `status = Eliminated` → `is-eliminated`, `data-status="eliminated"`, and
      `GHOST` in place of the HP number;
    - **no input produces `is-hit`** — assert `player_plaque` never emits it for
      any seat fixture, because "hit" is a transient event the client adds, not
      a projected state.
15. `test_plaque_carries_its_motion_anchor` — seat 0 → 
    `data-flight-anchor="plaque-seat-0"`, seat 7 → `plaque-seat-7`. And
    `deck_stack(Deck::Wine, 4)` → `data-flight-anchor="deck-wine"`;
    `discard_slot(3)` → `data-flight-anchor="discard"`.
16. `test_plaque_multi_deck` — a two-vessel seat renders two dots, `Beer +
    Liquor` in the deck names, `data-decks="beer,liquor"`, and a `lc-rule-2`
    split rule.
16b. `test_plaque_draw_badge` — a seat with `draws: 3` renders `.lc-draws`
    containing `3`; a seat with `draws: 0` renders no `.lc-draws` element at
    all. A badge reading `0` is noise on seven plaques at once.
17. `test_lc_banner_beat_hue_and_meta` — `round = 6, beat = Beat::Lock` →
    `lc-beat-violet`, `LOCK`, `ROUND 6`, `BEAT 4 OF 6`, `data-beat="lock"`,
    `data-round="6"`. Repeat for `Draw` → `lc-beat-amber` / `BEAT 1 OF 6` and
    `Deal` → `lc-beat-amber` / `BEAT 2 OF 6` (the inherited hue).
18. `test_lc_public_panel_carries_seq_and_no_hands` — from a state where every
    player holds `set_vessel`-dealt cards, `beat = Beat::Lock`: contains
    `data-seq` and `data-lc-banner`, and **none of** `beer-01`, `cider-01`,
    `wine-01`, `liquor-01`, `soft-01`, `Nudge`, `Sticky`.

- [ ] **Step 7: Commit**

```bash
git add drinkinggame/src/lc_render.rs drinkinggame/src/lib.rs
git commit -m "feat(drinks): Last Call component renderers to the §7.8 DOM contract"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

## Before this plan is done

- Every task is Class A or B, and every acceptance is a real command. **No task
  in this plan is Class C** — every Class C task in the slice lives in Plan A2.
- **Nothing is viewable.** No route beyond `/assets/lastcall.css`, no template,
  no JavaScript. If a task produced a page, it took work belonging to Plan
  A-vis. The single plan-end whole-diff review on the most capable model covers
  all three tasks.
- **No `hx-post`, `hx-get` or `onclick` appears anywhere in this plan's output.**
  The §7.8 contract is structure, never behaviour; interactions are slice 2 and
  3. Task 3's `test_no_builder_emits_behaviour` holds that line mechanically.
- Every §7.8 component root, required attribute and exposed attribute this plan
  owns is rendered and asserted, and the plaque, deck stack and discard slot
  carry their `data-flight-anchor` names. `felt` and `hand` are Plan A-vis's.
- `preview_state()` is defined here, is **not** `#[cfg(test)]`, and is asserted
  by `test_preview_state_covers_every_variant`. Plan A-vis consumes it and
  defines no fixtures of its own — one builder, so a test failure and a visual
  regression cannot disagree about what the fixture is (spec §8).
- No migration was written and `cargo sqlx prepare` was not run; neither is
  needed. Nothing in this plan reads or writes the database.
- Spec §2's "In" list maps as: (3) Task 1 · (4) Task 1 · (7) Tasks 2+3 — and
  the rest of (7) is **Plan A-vis**, (1), (2), (5), (6), (8) are **Plan A2**,
  (9), (10) are **Plan B**. §7.5 is Tasks 1+2+3, §7.6 is Tasks 2+3, §7.8 is
  Tasks 2+3. §7.7 is entirely **Plan A-vis**.
- Names and types the three later plans build against match what this plan
  produces: `LastCallState`, `PublicView`, `PublicSeat`, `Deck`, `Beat`, `Card`,
  `BackSize`, `preview_state()`, every `lc_render` builder, and the full CSS
  class contract.
