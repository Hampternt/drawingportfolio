# Last Call — Plan D: the loop engine (slice 3a)

> **For agentic workers:** REQUIRED SUB-SKILLS: `plan-economics` (this repo's
> task classes, review policy and plan sizing) then
> superpowers:subagent-driven-development to execute task-by-task.

**Goal:** Implement the six-beat round loop as pure state-machine transitions in
`last_call.rs` — arm / disarm / lock / reveal / resolve, damage and elimination,
round advance — killing every `NotImplemented` stub.

**Architecture:** `last_call.rs` stays a pure state machine, mirroring
`three_man.rs`: no I/O, no SQL, **no RNG** — card identity for draws is passed
in by callers exactly as dice values are in `three_man.rs`, and the shoe is a
count (`deck_counts`), not a pile. Locked-but-unrevealed plays live in a new
`locked_plays` field that `public_view()` never reads (spec §3.4.1); plays enter
`plays` only at the Lock→Reveal transition, the moment they become revealable.
`resolve()` owns the whole beat-6 program including the round rollover;
`advance_beat()` owns the other five advances and refuses at Resolve.

**Slice:** When this plan is done the engine plays a complete round end to end
under unit test: vessels feed a shoe, players arm/target/lock in secret, reveal
charges pulls and orders plays by spend, resolution moves HP, eliminates at 0,
expires effects, enforces the soft hand cap and rotates `first_seat` — and
`outcome()` reports the winner. Nothing user-visible changes: no route calls
any new transition yet. Plan E (slice 3b) wires routes, the beat ticker and SSE;
Plan F authors real cards against the op vocabulary defined here.

---

## Proposed design decisions — awaiting user review

DDv2 is silent or vague on all of these. Each is implemented as proposed below;
the user design-reviews this list after the plan executes. Every number is a
named constant so a playtest can move it.

- **D1 — Target assignment is a new transition, `set_target`.** `arm()`'s
  signature is final and carries no target, so targeting must be its own call;
  a `targets == "one"` card must have a target before `lock_in` accepts
  (`NeedsTarget` names the card, per 6.3's "naming the card").
- **D2 — Target classes:** `self` → the source (a "one" card MAY self-target);
  `one` → the chosen Alive seat; `all` → every Alive player **including** the
  source (the placeholder "all" cards read that way — "everyone feels better");
  `table` → no player subjects, resolves as a no-op until a table-rules system
  exists. In `Play.target`: `self` stores `Some(source_seat)`, `all`/`table`
  store `None`.
- **D3 — Payment vessel choice is deterministic greedy:** each card pays
  `pull_cost(cost, handicap)` from the vessel of its deck with the most
  `pulls_left` at that moment (tie → lowest vessel index), cards in arming
  order. The same simulation runs at arm (early feedback), at lock (6.3's
  mandated revalidation) and as the charge at reveal — vessels cannot change
  between lock and reveal, so the three always agree.
- **D4 — The ordering total (7.1) counts only pulls charged for plays.**
  Finish-&-draw empties a vessel but buys no tempo — 7.1's intent ties order to
  what the reveal makes public, which is play payment.
- **D5 — `resolve()` owns the round rollover** (rotate `first_seat`, `round +=
  1`, `beat = Draw`, reset per-round flags, reshuffle empty decks);
  `advance_beat()` at Resolve returns `Err(MustResolve)`. One beat per
  `advance_beat` call — Deal is a real beat the banner may show; Plan E's
  ticker advances it immediately (`duration_secs()` returns `None` for auto
  beats).
- **D6 — Shoe model:** `deck_counts` is the pile; card identity is supplied by
  the caller (the `three_man.rs` dice pattern), sampled from the catalog by
  Plan E's routes. A deck's shoe activates at `LC_DECK_SIZE` (40, placeholder)
  when the first vessel of that deck is registered. DDv2 §14's "cards in play +
  hands + discards = deck size" invariant is knowingly deferred to Plan F's
  real catalog — identity is virtual in the placeholder era.
- **D7 — Partial draws are legal at beat 1** when deck + reshuffled discard
  hold fewer than `DRAW_PER_VESSEL`: expected draw = `min(5, count)`. The
  "nobody draws a partial hand" adjudication governs *mid-round* empties, which
  cannot occur here (all draws are beat-1); demanding exactly 5 against the
  4-card placeholder catalog would deadlock finish-&-draw.
- **D8 — Placeholder card→effect mapping by `CardKind`:** Atk → immediate
  damage `cost × DMG_PER_COST (2)`; Buff → immediate heal `cost ×
  HEAL_PER_COST (2)`; Curse → a `Dot` effect, magnitude `cost ×
  DOT_PER_COST (1)` per round for `CURSE_ROUNDS (2)` rounds; Util and Reaction
  → no effect. The walkthrough's numbers are invented and were not copied.
- **D9 — Reaction cards cannot be armed** (`NotPlayable`): 7.3 makes them
  beat-5 cards and the response window is a later slice; arming them now would
  give them a beat-4 life the rules never granted.
- **D10 — Effect no-stack (TBD-8) = replace:** a new persistent effect with
  the same `(op, subject)` replaces the old one (fresh magnitude and expiry).
  Dots tick at each `resolve()` *after* their creation round through
  `expires_round`, then expire (remove when `expires_round <= round`, checked
  at the end of resolve). Effects on an eliminated player are removed.
- **D11 — HP clamps at 0** (display cleanliness; elimination triggers at 0).
  Elimination discards the player's hand to the decks' discard piles and
  removes their unresolved plays from the queue (7.6, "ghosts hold no cards").
- **D12 — Soft-cap enforcement is interim auto-discard, newest cards first,**
  down to `HAND_SOFT_CAP (12)` during resolve. 8.2's "choosing which" is a UI
  flow that does not exist yet; when a discard picker is designed, the engine
  grows a choice transition and this auto rule becomes its timeout fallback.
- **D13 — `first_seat` rotates `(first_seat + 1) % players.len()`,** not
  skipping eliminated seats — it is only a tie-break origin, and skipping
  would make rotation depend on elimination order.
- **D14 — The Diplomacy swap is not built.** Beat 3 is a timed no-op at the
  engine level (Plan E owns the timer); the face-down swap needs a
  consenting-pair UI and is deferred with the other content systems.
- **D15 — `set_vessel` is now Draw-beat-gated** (4.4's "round boundary") and
  keeps its placeholder opening deal (the deck's 4 catalog cards) until Plan F
  defines a real opening hand; it now also debits the shoe. Signature
  unchanged, so `lc_routes.rs` is untouched by it — game setup happens at
  round 1 `Beat::Draw`, so the existing setup flow still passes the gate.
- **D16 — `outcome()` returns `None` while fewer than 2 players are seated**
  (there is no game to win); on game over, `resolve()` completes resolution
  but skips the rollover — the state freezes at `Beat::Resolve` as the final
  tableau, and Plan E ends the game via the existing end route.
- **D17 — `LcPlayer.armed` changes type** from `Vec<Card>` to
  `Vec<ArmedCard { card, target }>` so a staged target travels with its card.
  Serde-safe: nothing has ever written a non-empty `armed` into a stored blob
  (grep-verified — no writer exists), and `[]` deserializes into any `Vec<T>`.
- **D18 — `seq` bumps on every successful mutating transition,** including
  `arm`/`disarm`/`set_target`. The bump is anonymous at the transport level
  and the arming player's own hand fragment needs the repaint signal.

---

## Global Constraints

Every task's requirements implicitly include this section.

### Scope — engine only

All work lands in `drinkinggame/src/last_call.rs` except **two named mechanical
edits** (both in Task 1, both forced by type changes, neither a behavior
change):

- `drinkinggame/src/lc_routes.rs:245-250` — the `set_vessel` error match is
  exhaustive over today's three `LcError` variants; its second arm becomes a
  `_` catch-all so the enum can grow.
- `drinkinggame/src/lc_render.rs` `ring_fixture()` (~line 1357) — the
  `PublicView` literal gains `outcome: None,`.

No routes, no SSE, no rendering logic, no templates, no CSS, no JS. If a task
finds itself editing a handler body or a builder's markup, it has crossed the
line and must stop and report. Plan E wires this engine to the room.

### Binding rules

- **Spec §3.4.1:** locked-but-unrevealed plays MUST NOT be stored in
  `LastCallState::plays`. They live in the new `locked_plays` field, which
  `public_view()` never reads. Before beat 5 the projection may see only the
  per-seat lock tick (`PublicSeat.locked`). Task 3 (the task whose `lock_in`
  stages plays) owns the MANDATORY test asserting a locked play is absent from
  `public_view()` output during beats 1–4. Task 4's Lock→Reveal transition is
  the only code that moves plays into `plays`.
- **No RNG in the state machine.** Random values are passed in by callers
  (`finish_and_draw` takes the drawn cards; `rng_seed` stays a stored replay
  hook). Mirrors `three_man.rs`.
- **Serde version-skew rules** (doc comment above `LastCallState`): new fields
  on `LastCallState` are safe (container-level `#[serde(default)]`); nested
  structs stay strict. The one nested change, `armed`'s element type, is safe
  per D17. `Effect.op: String` → `EffectOp` keeps the same snake_case JSON
  strings, so existing dev blobs parse.
- **seq discipline:** every successful mutating transition bumps `seq` exactly
  once; failed calls and idempotent replays bump nothing (precedent:
  `add_player`, `last_call.rs:446`).
- **Guard order is part of the interface.** Each transition documents its
  error-check order below; tests pin it, and Plan E's handlers map errors
  trusting it.
- Keep `drinkinggame` clippy-clean — the 17 pre-existing warnings are all in
  `drawingportfolio` and the count must not grow.

### The op vocabulary (Plan F authors real cards against this)

`EffectOp`, snake_case serde. "Persists" means it is stored as an `Effect` on
the room; immediate ops apply during resolution and are never stored.

| op | JSON | persists | semantics |
| --- | --- | --- | --- |
| `Damage` | `"damage"` | no | subtract `magnitude` from subject HP, shields first (in effect-creation order), clamp HP at 0, elimination check immediately (7.6) |
| `Heal` | `"heal"` | no | add `magnitude` to subject HP; **no ceiling** (TBD-3) |
| `Shield` | `"shield"` | yes | absorbs damage up to `magnitude` until `expires_round`; `magnitude` is consumed as it absorbs; removed at 0 |
| `Dot` | `"dot"` | yes | `magnitude` damage to subject at each `resolve()` after its creation round, through `expires_round` |

### Constants (all in `last_call.rs`, all playtest-movable)

```rust
pub const DRAW_SECS: u16 = 30;      // DDv2 §5 beat 1
pub const DIPLOMACY_SECS: u16 = 60; // DDv2 §5 beat 3, TBD-6
pub const LOCK_SECS: u16 = 45;      // DDv2 §5 beat 4
pub const REVEAL_SECS: u16 = 20;    // DDv2 §5 beat 5
pub const DRAW_PER_VESSEL: usize = 5; // DDv2 §4.3, TBD-4
pub const HAND_SOFT_CAP: usize = 12;  // DDv2 §8.2, TBD-2
pub const LC_DECK_SIZE: u16 = 40;   // placeholder shoe size (D6) — Plan F resets
pub const DMG_PER_COST: i32 = 2;    // placeholder mapping (D8)
pub const HEAL_PER_COST: i32 = 2;   // placeholder mapping (D8)
pub const DOT_PER_COST: i32 = 1;    // placeholder mapping (D8)
pub const CURSE_ROUNDS: u32 = 2;    // placeholder mapping (D8)
```

TBD-1 (`STARTING_HP` 15) already exists. TBD-3 (no healing ceiling) is the
absence of a clamp, documented at the heal site. TBD-5 (one finish-&-draw per
round) is a rule, enforced via the `drawing` flag. TBD-7 (reactions) is out of
scope with the reaction system.

### Verification

**Verification for every task:** `./scripts/verify.sh` — all green, output
quoted in the report. Never a bare `cargo test` (it runs 53 of 371 and skips
`drinkinggame` entirely).

**Baseline before Task 1:** green, **371 tests**, **17 distinct** clippy
warnings, all in `drawingportfolio`. The verify log greps higher — rustc
dead-code warnings appear twice and four lines are rollup summaries; compare
against 17.

**Browser checkpoints: none.** Nothing visual ships in this plan — every
deliverable is a pure function pinned by unit tests. **No `cargo sqlx
prepare`** — no migration, and `drinkinggame` uses runtime-checked queries.

SDD ledger: `.superpowers/sdd/2026-08-11-last-call-plan-d-loop-engine/progress.md`.

---

### Task 1: Vocabulary, errors, constants — and the `from_json` seat cap

**Class:** B (logic, tests specified below)

**Why this class:** type definitions plus two small pure functions
(`outcome()`, the deserialization cap), each with expected values written out
here; the two out-of-file edits are compile-mechanical.

**Files:**
- Modify: `drinkinggame/src/last_call.rs`
- Modify: `drinkinggame/src/lc_routes.rs:245-250` (match arm only)
- Modify: `drinkinggame/src/lc_render.rs` (`ring_fixture()`, one line)
- Test: `drinkinggame/src/last_call.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: existing `LastCallState`, `Effect`, `PublicView`, `MAX_SEATS`.
- Produces (later tasks and Plans E/F build against these — exact):

```rust
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EffectOp { Damage, Heal, Shield, Dot }
// Effect.op changes from String to EffectOp — same JSON strings.

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LcOutcome {
    Winner(usize), // the winning seat
    Draw,          // all remaining players are ghosts (DDv2 9.3)
}

// PublicView gains (projection is engine work; Plan E renders it):
pub outcome: Option<LcOutcome>,

/// Plan E queries this after resolve() to decide end-of-game handling.
pub fn outcome(&self) -> Option<LcOutcome>;

impl Beat {
    /// None for the auto beats (Deal, Resolve). Plan E's ticker consumes.
    pub fn duration_secs(self) -> Option<u16>;
}

#[derive(Debug, PartialEq, Eq)]
pub enum LcError {
    NotSeated,
    BadHandicap,
    WrongBeat,           // action not legal in the current beat
    NotAlive,            // eliminated players act on nothing
    AlreadyLocked,       // arm/disarm/set_target after lock_in
    UnknownCard,         // card_id not in the expected zone
    NotPlayable,         // a Reaction card at arm time (D9)
    CantAfford(String),  // lock/arm validation, naming the card (DDv2 6.3)
    NeedsTarget(String), // a targets=="one" card with no target at lock
    BadTarget,           // set_target: bad seat / dead seat / class mismatch
    BadDraw,             // finish_and_draw: bad vessel, second draw, bad batch
    MustResolve,         // advance_beat at Resolve — call resolve() instead
    NotImplemented,      // dies in Task 5 with the last stub
}
```

- [ ] **Step 1: Add the enums, constants and `Beat::duration_secs`**

The constants block from Global Constraints, verbatim. `duration_secs`:

```rust
pub fn duration_secs(self) -> Option<u16> {
    match self {
        Beat::Draw => Some(DRAW_SECS),
        Beat::Deal => None,
        Beat::Diplomacy => Some(DIPLOMACY_SECS),
        Beat::Lock => Some(LOCK_SECS),
        Beat::Reveal => Some(REVEAL_SECS),
        Beat::Resolve => None,
    }
}
```

Change `Effect.op` to `EffectOp` and update the one existing constructor in
`test_serde_round_trip` (`op: "damage".to_string()` → `op: EffectOp::Dot` —
Dot, because Dot is what a persisted `Effect` will actually carry).

- [ ] **Step 2: Extend `LcError`, catch-all the routes match**

Add the new variants (keep `NotImplemented` for now — three stubs still return
it until Tasks 4–5). In `lc_routes.rs:247` replace the second arm:

```rust
LcError::NotSeated => GameError::NotYourCall.into_response(),
_ => StatusCode::UNPROCESSABLE_ENTITY.into_response(),
```

(One arm change; the handler's behavior for both existing error values is
identical to before.)

- [ ] **Step 3: `outcome()` and the `PublicView` projection**

```rust
/// DDv2 9.3. None while the game is undecided — or while fewer than two
/// players are seated, because a table of one has no game to win (D16).
pub fn outcome(&self) -> Option<LcOutcome> {
    if self.players.len() < 2 {
        return None;
    }
    let mut alive = self.players.iter().filter(|p| p.status == Status::Alive);
    match (alive.next(), alive.next()) {
        (Some(p), None) => Some(LcOutcome::Winner(p.seat)),
        (None, _) => Some(LcOutcome::Draw),
        _ => None,
    }
}
```

`public_view()` gains `outcome: self.outcome(),`. In `lc_render.rs`'s
`ring_fixture()` add `outcome: None,` to the `PublicView` literal. (That
fixture and `PublicSeat` struct-update expressions are the only out-of-engine
constructors — grep-verified.)

- [ ] **Step 4: Cap `from_json` — the carried pre-deploy item**

`from_json` is the third path into `players` and the only uncapped one. After
parsing, truncate:

```rust
pub fn from_json(s: &str) -> Self {
    let mut st: LastCallState =
        serde_json::from_str(s).expect("valid LastCallState JSON");
    // The third path into `players` — LastCallState::new and add_player
    // both cap at MAX_SEATS; a blob persisted by a pre-ceiling binary must
    // not deserialize past the ring (seat_pos renders short and a real
    // player's plaque silently vanishes).
    st.players.truncate(MAX_SEATS);
    st
}
```

- [ ] **Step 5: Tests**

```rust
#[test]
fn test_from_json_caps_players_at_max_seats() {
    let mut st = LastCallState::new(
        (1..=8).map(|i| (i, format!("p{i}"))).collect(), 42);
    // A ninth player, hand-built — the shape only a pre-ceiling blob has.
    let mut ninth = st.players[0].clone();
    ninth.seat = 8;
    ninth.player_id = 9;
    st.players.push(ninth);
    let loaded = LastCallState::from_json(&st.to_json());
    assert_eq!(loaded.players.len(), MAX_SEATS);
    assert!(loaded.seat_of(9).is_none());
}

#[test]
fn test_beat_durations() {
    assert_eq!(
        Beat::ORDER.map(|b| b.duration_secs()),
        [Some(30), None, Some(60), Some(45), Some(20), None]
    );
}

#[test]
fn test_effect_op_serde_names() {
    assert_eq!(serde_json::to_string(&EffectOp::Dot).unwrap(), "\"dot\"");
    assert_eq!(serde_json::to_string(&EffectOp::Damage).unwrap(), "\"damage\"");
}

#[test]
fn test_outcome_detection() {
    let mut st = seated(); // 3 players
    assert_eq!(st.outcome(), None);
    st.players[1].status = Status::Eliminated;
    assert_eq!(st.outcome(), None); // two still alive
    st.players[2].status = Status::Eliminated;
    assert_eq!(st.outcome(), Some(LcOutcome::Winner(0)));
    assert_eq!(st.public_view().outcome, Some(LcOutcome::Winner(0)));
    st.players[0].status = Status::Eliminated;
    assert_eq!(st.outcome(), Some(LcOutcome::Draw));

    let solo = LastCallState::new(vec![(1, "alice".into())], 42);
    assert_eq!(solo.outcome(), None); // no game to win (D16)
}
```

- [ ] **Step 6: Commit**

```bash
git add drinkinggame/src/last_call.rs drinkinggame/src/lc_routes.rs drinkinggame/src/lc_render.rs
git commit -m "feat(lastcall): loop vocabulary — EffectOp, LcOutcome, LcError, beat durations; cap from_json at MAX_SEATS"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

### Task 2: The Draw beat — the shoe, `finish_and_draw`, `set_vessel` rework

**Class:** B (logic, tests specified below)

**Why this class:** pure vessel/shoe arithmetic; every case and expected value
is written below.

**Files:**
- Modify: `drinkinggame/src/last_call.rs`
- Test: `drinkinggame/src/last_call.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: Task 1's `LcError::{WrongBeat, NotAlive, BadDraw}`, `LC_DECK_SIZE`,
  `DRAW_PER_VESSEL`.
- Produces (exact — Plan E's draw route builds against this):

```rust
/// DDv2 4.3 finish-&-draw, beat 1 only, once per player per round (TBD-5).
/// `drawn` is decided by the caller (no RNG here): its length MUST equal
/// min(DRAW_PER_VESSEL, shoe count for the vessel's deck) and every card
/// MUST belong to that deck. Empties-and-refills the vessel to pulls_max,
/// debits the shoe, extends the hand, sets `drawing` and `draws_this_round`.
pub fn finish_and_draw(
    &mut self,
    player_id: i64,
    vessel_idx: usize,
    drawn: Vec<Card>,
) -> Result<(), LcError>;
// set_vessel: signature unchanged; now Draw-beat-gated, activates the shoe,
// debits it by the cards actually dealt.
```

- [ ] **Step 1: Rework `set_vessel`**

Guard order: `NotSeated` → `NotAlive` → `WrongBeat` (beat must be
`Beat::Draw`, D15 — game setup happens at round 1 Draw, so the existing route
flow is unaffected). Then, before mutating: if **no player holds a vessel of
`deck`** (scan all players), set that deck's `deck_counts` entry to
`LC_DECK_SIZE` (shoe activation, D6). Keep the existing same-deck
replace-and-redeal behavior verbatim, but count the cards actually pushed to
the hand and subtract that from the deck's count (`saturating_sub`). `seq`
bump stays as-is.

- [ ] **Step 2: `finish_and_draw`**

Guard order: `NotSeated` → `NotAlive` → `WrongBeat` (must be `Beat::Draw`) →
`BadDraw` if `vessel_idx >= vessels.len()` → `BadDraw` if `drawing` is already
true (TBD-5: `drawing` means "already used finish-&-draw this round"; it is the
plaque's pulse flag and is cleared when Draw ends) → `BadDraw` unless
`drawn.len() == min(DRAW_PER_VESSEL, count(deck))` and every `drawn[i].deck`
matches the vessel's deck (D7: the min is what makes a short shoe legal).

Then: `vessel.pulls_left = vessel.pulls_max` (empty it, open a fresh one);
shoe count `-= drawn.len()`; `hand.extend(drawn)`; `draws_this_round +=` the
drawn count; `drawing = true`; `seq += 1`.

- [ ] **Step 3: Reorder `preview_state`**

`set_vessel` is now Draw-gated and `preview_state` sets `st.beat = Beat::Lock`
*before* its `set_vessel` calls. Move the `st.beat = Beat::Lock;` line down to
just before the `st.deck_counts = vec![...]` overwrite (after the last
`set_vessel` call, seat 8's). Nothing else moves; the manual `deck_counts`
overwrite already masks the new shoe arithmetic, so
`test_preview_state_covers_every_variant` and every render test that consumes
the fixture are unchanged.

- [ ] **Step 4: Tests**

```rust
#[test]
fn test_set_vessel_activates_and_debits_the_shoe() {
    let mut st = seated();
    st.set_vessel(1, Deck::Beer, "can").unwrap();
    // 40 in, 4 catalog cards dealt out.
    assert_eq!(deck_count(&st, Deck::Beer), LC_DECK_SIZE - 4); // 36
    st.set_vessel(2, Deck::Beer, "can").unwrap();
    // No reactivation — same shoe, four more cards dealt.
    assert_eq!(deck_count(&st, Deck::Beer), LC_DECK_SIZE - 8); // 32
    // Same-deck re-registration replaces the vessel; the dedupe deals 0
    // new cards, so the shoe is untouched.
    st.set_vessel(1, Deck::Beer, "bigger can").unwrap();
    assert_eq!(deck_count(&st, Deck::Beer), LC_DECK_SIZE - 8);
    assert_eq!(st.players[0].vessels.len(), 1);
}

#[test]
fn test_set_vessel_outside_draw_is_rejected() {
    let mut st = seated();
    st.beat = Beat::Lock;
    assert_eq!(st.set_vessel(1, Deck::Beer, "can"), Err(LcError::WrongBeat));
}

#[test]
fn test_finish_and_draw_refills_and_draws() {
    let mut st = seated();
    st.set_vessel(1, Deck::Beer, "can").unwrap(); // shoe 36, hand 4, 8/8
    st.players[0].vessels[0].pulls_left = 2;      // most of the can is gone
    let before_seq = st.seq;
    let mut drawn = crate::lc_cards::deck_cards(Deck::Beer); // 4
    drawn.push(crate::lc_cards::card_by_id("beer-01").unwrap()); // 5 — dups fine
    st.finish_and_draw(1, 0, drawn).unwrap();
    let p = &st.players[0];
    assert_eq!(p.vessels[0].pulls_left, 8); // fresh can
    assert_eq!(p.hand.len(), 9);
    assert_eq!(p.draws_this_round, 5);
    assert!(p.drawing);
    assert_eq!(deck_count(&st, Deck::Beer), 31);
    assert_eq!(st.seq, before_seq + 1);
}

#[test]
fn test_one_finish_and_draw_per_round() { // TBD-5
    let mut st = seated();
    st.set_vessel(1, Deck::Beer, "can").unwrap();
    let mut drawn = crate::lc_cards::deck_cards(Deck::Beer);
    drawn.push(crate::lc_cards::card_by_id("beer-01").unwrap());
    st.finish_and_draw(1, 0, drawn.clone()).unwrap();
    assert_eq!(st.finish_and_draw(1, 0, drawn), Err(LcError::BadDraw));
}

#[test]
fn test_finish_and_draw_validates_the_batch() {
    let mut st = seated();
    st.set_vessel(1, Deck::Beer, "can").unwrap(); // shoe 36 → expects 5
    // Too few:
    assert_eq!(
        st.finish_and_draw(1, 0, crate::lc_cards::deck_cards(Deck::Beer)),
        Err(LcError::BadDraw)
    );
    // Right count, wrong deck in the batch:
    let mut bad = crate::lc_cards::deck_cards(Deck::Beer);
    bad.push(crate::lc_cards::card_by_id("cider-01").unwrap());
    assert_eq!(st.finish_and_draw(1, 0, bad), Err(LcError::BadDraw));
    // Bad vessel index:
    assert_eq!(st.finish_and_draw(1, 5, vec![]), Err(LcError::BadDraw));
    // Wrong beat:
    st.beat = Beat::Deal;
    assert_eq!(st.finish_and_draw(1, 0, vec![]), Err(LcError::WrongBeat));
}

#[test]
fn test_short_shoe_draws_partial() { // D7
    let mut st = seated();
    st.set_vessel(1, Deck::Beer, "can").unwrap();
    set_deck_count(&mut st, Deck::Beer, 3); // shoe nearly out → expects 3
    let mut five = crate::lc_cards::deck_cards(Deck::Beer);
    five.push(crate::lc_cards::card_by_id("beer-01").unwrap());
    assert_eq!(st.finish_and_draw(1, 0, five), Err(LcError::BadDraw));
    let three = crate::lc_cards::deck_cards(Deck::Beer)[..3].to_vec();
    st.finish_and_draw(1, 0, three).unwrap();
    assert_eq!(deck_count(&st, Deck::Beer), 0);
    assert_eq!(st.players[0].hand.len(), 7);
}
```

`deck_count` / `set_deck_count` are 3-line test helpers over the
`Vec<(Deck, u16)>` lookup — write them once at the top of the tests module.

- [ ] **Step 5: Commit**

```bash
git add drinkinggame/src/last_call.rs
git commit -m "feat(lastcall): the Draw beat — shoe activation, finish-and-draw, Draw-gated set_vessel"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

### Task 3: Arm, disarm, target, lock — staging in secret

**Class:** B (logic, tests specified below — including the mandatory §3.4.1
secrecy test, which is encodable as a JSON-absence assertion and is therefore
a test-is-the-spec case, not a reviewer case)

**Files:**
- Modify: `drinkinggame/src/last_call.rs`
- Test: `drinkinggame/src/last_call.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: Task 1's `LcError` variants; `pull_cost` (exists).
- Produces (exact):

```rust
/// A staged card: identity plus its declared target. Never projected.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ArmedCard {
    pub card: Card,
    pub target: Option<usize>,
}
// LcPlayer.armed: Vec<ArmedCard>   (was Vec<Card> — D17)
// LastCallState gains (container serde(default) makes this skew-safe):
//   pub locked_plays: Vec<Play>,   // §3.4.1: public_view() NEVER reads this

pub fn arm(&mut self, player_id: i64, card_id: &str) -> Result<(), LcError>;
pub fn disarm(&mut self, player_id: i64, card_id: &str) -> Result<(), LcError>;
pub fn set_target(
    &mut self,
    player_id: i64,
    card_id: &str,
    target: Option<usize>,
) -> Result<(), LcError>;
pub fn lock_in(&mut self, player_id: i64) -> Result<(), LcError>;

/// Private helper, shared with Task 4's reveal charge: the deterministic
/// greedy payment simulation (D3). Returns, per armed card in order, the
/// (vessel index, pulls) it pays — or CantAfford naming the first card
/// that cannot pay.
fn payment_plan(player: &LcPlayer) -> Result<Vec<(usize, u8)>, LcError>;
```

- [ ] **Step 1: `arm`**

Guard order: `NotSeated` → `NotAlive` → `WrongBeat` (must be `Beat::Lock` —
DDv2 §5 beat 4 is where arming lives) → `AlreadyLocked` → `UnknownCard` (id
not in hand) → `NotPlayable` if `card.kind == CardKind::Reaction` (D9) →
`CantAfford(card_id)` if `payment_plan` over current armed **plus this card**
fails (4.2, checked early for UX; 6.3's lock-time check remains authoritative).
Success: move the card from `hand` to `armed` as
`ArmedCard { card, target: None }`, `seq += 1`.

`payment_plan` (D3): simulate per card in arming order — pick the vessel of
`card.deck` with the greatest remaining simulated `pulls_left` (tie → lowest
index); deduct `pull_cost(card.cost, handicap_pct)`; if no vessel of that deck
can cover the cost, `Err(CantAfford(card.id.clone()))`.

- [ ] **Step 2: `disarm` and `set_target`**

`disarm` guards: `NotSeated` → `NotAlive` → `WrongBeat` → `AlreadyLocked` →
`UnknownCard` (not armed). Success: move the card back to `hand` (target
dropped with the `ArmedCard`), `seq += 1`.

`set_target` guards: `NotSeated` → `NotAlive` → `WrongBeat` → `AlreadyLocked`
→ `UnknownCard` (not armed) → `BadTarget` per D2: `targets == "one"` requires
`Some(seat)` where the seat exists and is `Alive` (self-targeting allowed);
every other target class requires `None`. Success: set the `ArmedCard`'s
target, `seq += 1`.

- [ ] **Step 3: `lock_in`**

Guards: `NotSeated` → `NotAlive` → `WrongBeat` → already locked → `Ok(())`
with **no** seq bump (idempotent replay, the `add_player` precedent). Then
validate in arming order: first `NeedsTarget(card_id)` for any
`targets == "one"` card without a target, then `payment_plan` →
`CantAfford(card_id)` (6.3: "rejects the lock ... naming the card"). On
success, for each `ArmedCard` in order push onto **`locked_plays`**:

```rust
Play {
    card,
    source_seat: seat,
    target: match card.targets.as_str() {
        "self" => Some(seat),          // D2
        "one" => armed.target,         // validated Some
        _ => None,                     // all / table
    },
    paid_from: card.deck,
    order_key: 0,                      // set at reveal (DDv2 §1), not here
}
```

then clear `armed`, set `locked = true`, `seq += 1`. Pulls are **not**
charged — payment happens at reveal (6.4); a test pins it. Locking zero cards
is legal (play nothing).

Delete `test_stubs_are_not_implemented` (arm/disarm/lock_in are now real; the
two remaining stubs die in Tasks 4–5 and each replacement task carries its own
guard tests).

- [ ] **Step 4: Tests**

Shared fixture at the top of the tests module:

```rust
/// alice(1)/Beer, bob(2)/Cider, cara(3)/Soft — vessels registered at Draw,
/// then moved to the Lock beat.
fn at_lock() -> LastCallState {
    let mut st = seated();
    st.set_vessel(1, Deck::Beer, "can").unwrap();
    st.set_vessel(2, Deck::Cider, "bottle").unwrap();
    st.set_vessel(3, Deck::Soft, "glass").unwrap();
    st.beat = Beat::Lock;
    st
}
```

```rust
#[test]
fn test_arm_moves_hand_to_armed() {
    let mut st = at_lock();
    let before = st.seq;
    st.arm(1, "beer-01").unwrap();
    assert_eq!(st.players[0].hand.len(), 3);
    assert_eq!(st.players[0].armed.len(), 1);
    assert_eq!(st.players[0].armed[0].card.id, "beer-01");
    assert_eq!(st.players[0].armed[0].target, None);
    assert_eq!(st.seq, before + 1);
}

#[test]
fn test_arm_guard_order() {
    let mut st = at_lock();
    assert_eq!(st.arm(999, "beer-01"), Err(LcError::NotSeated));
    assert_eq!(st.arm(1, "nope"), Err(LcError::UnknownCard));
    assert_eq!(st.arm(3, "soft-04"), Err(LcError::NotPlayable)); // Reaction, D9
    st.players[0].status = Status::Eliminated;
    assert_eq!(st.arm(1, "beer-01"), Err(LcError::NotAlive));
    st.players[0].status = Status::Alive;
    st.beat = Beat::Draw;
    assert_eq!(st.arm(1, "beer-01"), Err(LcError::WrongBeat));
}

#[test]
fn test_arm_affordability_is_aggregate() {
    let mut st = at_lock();
    st.players[0].vessels[0].pulls_left = 2;
    st.arm(1, "beer-01").unwrap(); // cost 1, plan: 1 of 2
    assert_eq!(
        st.arm(1, "beer-02"), // cost 2, total 3 > 2
        Err(LcError::CantAfford("beer-02".into()))
    );
    // Handicap inflates the check (4.2's cost × handicap):
    let mut st = at_lock();
    st.players[0].vessels[0].pulls_left = 2;
    st.set_handicap(1, 150).unwrap(); // pull_cost(2,150) = 3
    assert_eq!(st.arm(1, "beer-02"), Err(LcError::CantAfford("beer-02".into())));
}

#[test]
fn test_disarm_returns_the_card() {
    let mut st = at_lock();
    st.arm(1, "beer-01").unwrap();
    st.set_target(1, "beer-01", Some(1)).unwrap();
    st.disarm(1, "beer-01").unwrap();
    assert_eq!(st.players[0].hand.len(), 4);
    assert!(st.players[0].armed.is_empty());
    assert_eq!(st.disarm(1, "beer-01"), Err(LcError::UnknownCard));
}

#[test]
fn test_set_target_classes() { // D2
    let mut st = at_lock();
    st.arm(1, "beer-01").unwrap(); // targets "one"
    assert_eq!(st.set_target(1, "beer-01", None), Err(LcError::BadTarget));
    assert_eq!(st.set_target(1, "beer-01", Some(7)), Err(LcError::BadTarget));
    st.players[1].status = Status::Eliminated;
    assert_eq!(st.set_target(1, "beer-01", Some(1)), Err(LcError::BadTarget));
    st.players[1].status = Status::Alive;
    st.set_target(1, "beer-01", Some(0)).unwrap(); // self-target a "one": legal
    st.set_target(1, "beer-01", Some(1)).unwrap(); // retargeting: legal
    st.arm(1, "beer-03").unwrap(); // targets "self"
    assert_eq!(st.set_target(1, "beer-03", Some(1)), Err(LcError::BadTarget));
    st.set_target(1, "beer-03", None).unwrap();
}

#[test]
fn test_lock_in_stages_plays_and_pays_nothing() {
    let mut st = at_lock();
    st.arm(1, "beer-01").unwrap();
    st.set_target(1, "beer-01", Some(1)).unwrap();
    st.arm(1, "beer-03").unwrap(); // "self"
    st.lock_in(1).unwrap();
    let p = &st.players[0];
    assert!(p.locked);
    assert!(p.armed.is_empty());
    assert_eq!(p.vessels[0].pulls_left, 8); // payment at reveal (6.4)
    assert!(st.plays.is_empty());           // §3.4.1
    assert_eq!(st.locked_plays.len(), 2);
    assert_eq!(st.locked_plays[0].card.id, "beer-01");
    assert_eq!(st.locked_plays[0].target, Some(1));
    assert_eq!(st.locked_plays[1].target, Some(0)); // self → own seat (D2)
    assert_eq!(st.locked_plays[1].order_key, 0);    // set at reveal, not here
    // Idempotent replay: Ok, no bump, nothing re-staged.
    let seq = st.seq;
    st.lock_in(1).unwrap();
    assert_eq!((st.seq, st.locked_plays.len()), (seq, 2));
}

#[test]
fn test_lock_in_names_the_failing_card() { // DDv2 6.3
    let mut st = at_lock();
    st.arm(1, "beer-01").unwrap(); // "one", no target yet
    assert_eq!(st.lock_in(1), Err(LcError::NeedsTarget("beer-01".into())));
    st.set_target(1, "beer-01", Some(1)).unwrap();
    st.arm(1, "beer-02").unwrap();
    st.set_target(1, "beer-02", Some(1)).unwrap();
    st.players[0].vessels[0].pulls_left = 2; // 1+2=3 > 2 now
    assert_eq!(st.lock_in(1), Err(LcError::CantAfford("beer-02".into())));
    assert!(!st.players[0].locked);
    assert_eq!(st.players[0].armed.len(), 2); // rejection stages nothing
}

/// MANDATORY (spec §3.4.1): the function that stages plays owns this test.
/// A locked play is invisible to the projection during beats 1–4; only the
/// lock tick is public.
#[test]
fn test_a_locked_play_is_absent_from_public_view_before_reveal() {
    let mut st = at_lock();
    st.arm(1, "beer-01").unwrap();
    st.set_target(1, "beer-01", Some(1)).unwrap();
    st.lock_in(1).unwrap();
    assert!(st.plays.is_empty());
    assert_eq!(st.locked_plays.len(), 1);
    for beat in [Beat::Draw, Beat::Deal, Beat::Diplomacy, Beat::Lock] {
        st.beat = beat;
        let view = st.public_view();
        assert!(view.revealed.is_empty(), "beat={beat:?}");
        let json = serde_json::to_string(&view).unwrap();
        assert!(!json.contains("beer-01"), "beat={beat:?}");
        assert!(!json.contains("Nudge"), "beat={beat:?}");
        assert!(view.seats[0].locked, "the lock tick IS public, beat={beat:?}");
    }
}

#[test]
fn test_acting_after_lock_is_rejected() {
    let mut st = at_lock();
    st.lock_in(1).unwrap(); // locking nothing is legal
    assert_eq!(st.arm(1, "beer-01"), Err(LcError::AlreadyLocked));
    assert_eq!(st.disarm(1, "beer-01"), Err(LcError::AlreadyLocked));
    assert_eq!(st.set_target(1, "beer-01", None), Err(LcError::AlreadyLocked));
}
```

Also update `public_view()`'s stale doc comment: the "safe today only because
the stubs are `NotImplemented`" paragraph is replaced by a sentence pointing at
`locked_plays` and this test.

- [ ] **Step 5: Commit**

```bash
git add drinkinggame/src/last_call.rs
git commit -m "feat(lastcall): arm/disarm/target/lock — plays stage into locked_plays, invisible to public_view"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

### Task 4: `advance_beat` — and the reveal that charges and orders

**Class:** B (logic, tests specified below)

**Files:**
- Modify: `drinkinggame/src/last_call.rs`
- Test: `drinkinggame/src/last_call.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: Task 3's `locked_plays`, `ArmedCard`, `payment_plan`.
- Produces (exact — Plan E's ticker calls this):

```rust
/// One beat forward. Draw→Deal clears `drawing`; Lock→Reveal is the reveal:
/// unlocked players' armed cards return to hand (DDv2 §12, disconnect at
/// lock), locked plays are charged (6.4) and moved into `plays` with
/// order_key computed (7.1/7.2). At Resolve returns Err(MustResolve) —
/// resolve() owns the rollover (D5). Bumps seq on success.
pub fn advance_beat(&mut self) -> Result<(), LcError>;
```

- [ ] **Step 1: The five advances**

`advance_beat`: if `beat == Beat::Resolve` return `Err(LcError::MustResolve)`.
Otherwise `beat = beat.next()`, `seq += 1`, plus per-edge work:

- **Draw→Deal:** set every player's `drawing = false` (the pulse ends with the
  beat; the flag's TBD-5 duty is over for the round).
- **Deal→Diplomacy, Diplomacy→Lock, Reveal→Resolve:** no extra work (events,
  tabs, the swap and the reaction window are hollow systems — D14, D9).
- **Lock→Reveal:** the reveal, Step 2.

- [ ] **Step 2: The reveal (Lock→Reveal edge)**

In this order:

1. **Unlocked players play nothing** (§12): for every Alive player with
   `locked == false`, move each `ArmedCard.card` back to `hand`, clear
   `armed`. No charge.
2. **Charge pulls** (6.4): for each locked player, run `payment_plan` and
   apply it — deduct each card's pulls from its planned vessel. Vessels
   cannot change between lock and reveal (arming and drawing are other
   beats), so the plan `lock_in` validated cannot fail here; `expect` with a
   message saying exactly that.
3. **Order** (7.1/7.2): compute each locked player's round total = sum of
   `pull_cost(card.cost, handicap_pct)` over their staged plays (D4). Stable
   sort `locked_plays` by
   `(Reverse(total[source_seat]), (source_seat + n - first_seat) % n)` — the
   stable sort preserves each player's arming order, which is 7.2's
   within-player rule. Assign `order_key = i as u32 + 1` in sorted order.
4. **Flip everything at once:** `self.plays = std::mem::take(&mut self.locked_plays)`.
   This is the single point where plays become revealable, and `beat` is
   already `Reveal` when `public_view()` next runs.

- [ ] **Step 3: Tests**

```rust
/// alice locks beer-01(→bob) then beer-02(→bob): 3 pulls. bob locks
/// cider-04(→alice): 3 pulls. cara arms soft-01 but never locks.
fn locked_table() -> LastCallState {
    let mut st = at_lock();
    st.arm(1, "beer-01").unwrap();
    st.set_target(1, "beer-01", Some(1)).unwrap();
    st.arm(1, "beer-02").unwrap();
    st.set_target(1, "beer-02", Some(1)).unwrap();
    st.lock_in(1).unwrap();
    st.arm(2, "cider-04").unwrap();
    st.set_target(2, "cider-04", Some(0)).unwrap();
    st.lock_in(2).unwrap();
    st.arm(3, "soft-01").unwrap();
    st
}

#[test]
fn test_advance_walks_the_beats_and_refuses_resolve() {
    let mut st = seated();
    for expected in [Beat::Deal, Beat::Diplomacy, Beat::Lock, Beat::Reveal, Beat::Resolve] {
        let seq = st.seq;
        st.advance_beat().unwrap();
        assert_eq!(st.beat, expected);
        assert_eq!(st.seq, seq + 1);
    }
    assert_eq!(st.advance_beat(), Err(LcError::MustResolve)); // D5
}

#[test]
fn test_draw_to_deal_clears_drawing() {
    let mut st = seated();
    st.players[0].drawing = true;
    st.advance_beat().unwrap();
    assert!(!st.players[0].drawing);
}

#[test]
fn test_reveal_charges_orders_and_flips() {
    let mut st = locked_table();
    st.advance_beat().unwrap(); // Lock → Reveal
    assert_eq!(st.beat, Beat::Reveal);
    assert!(st.locked_plays.is_empty());
    assert_eq!(st.players[0].vessels[0].pulls_left, 5);  // Beer 8-3
    assert_eq!(st.players[1].vessels[0].pulls_left, 7);  // Cider 10-3
    assert_eq!(st.players[2].vessels[0].pulls_left, 6);  // cara never locked
    // cara's armed card went home, uncharged (§12):
    assert_eq!(st.players[2].hand.len(), 4);
    assert!(st.players[2].armed.is_empty());
    // 3 = 3 tie → seat order from first_seat 0 → alice first, arming order:
    assert_eq!(
        st.plays.iter().map(|p| (p.card.id.as_str(), p.order_key)).collect::<Vec<_>>(),
        vec![("beer-01", 1), ("beer-02", 2), ("cider-04", 3)]
    );
    // And the projection now — and only now — carries identity:
    let json = serde_json::to_string(&st.public_view()).unwrap();
    assert!(json.contains("Nudge"));
}

#[test]
fn test_bigger_spender_acts_first() { // 7.1
    let mut st = at_lock();
    st.arm(1, "beer-01").unwrap(); // alice spends 1
    st.set_target(1, "beer-01", Some(1)).unwrap();
    st.lock_in(1).unwrap();
    st.arm(2, "cider-04").unwrap(); // bob spends 3
    st.set_target(2, "cider-04", Some(0)).unwrap();
    st.lock_in(2).unwrap();
    st.advance_beat().unwrap();
    assert_eq!(st.plays[0].card.id, "cider-04");
    assert_eq!(st.plays[0].order_key, 1);
}

#[test]
fn test_first_seat_breaks_the_tie() { // 7.2
    let mut st = locked_table();
    st.first_seat = 1;
    st.advance_beat().unwrap();
    assert_eq!(st.plays[0].card.id, "cider-04"); // bob's seat leads now
}

#[test]
fn test_handicap_inflates_the_charge() { // §11: cost only, rounded up
    let mut st = at_lock();
    st.set_handicap(1, 150).unwrap();
    st.arm(1, "beer-01").unwrap(); // pull_cost(1,150) = 2
    st.set_target(1, "beer-01", Some(1)).unwrap();
    st.lock_in(1).unwrap();
    st.advance_beat().unwrap();
    assert_eq!(st.players[0].vessels[0].pulls_left, 6); // 8 - 2
}
```

- [ ] **Step 4: Commit**

```bash
git add drinkinggame/src/last_call.rs
git commit -m "feat(lastcall): advance_beat — the reveal charges pulls, orders by spend, flips plays public"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

### Task 5: `resolve` — damage, elimination, effects, the rollover

**Class:** B (logic, tests specified below)

**Why this class:** the largest task, but every rule is a pure function with
expected values below; nothing here is concurrency, auth or broadcast.

**Files:**
- Modify: `drinkinggame/src/last_call.rs`
- Test: `drinkinggame/src/last_call.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: Task 1's `EffectOp`/`LcOutcome`/`outcome()`, Task 4's ordered
  `plays`, the D8 constants.
- Produces: `pub fn resolve(&mut self) -> Result<(), LcError>` — the beat-6
  program plus the round rollover (D5). Removes `LcError::NotImplemented` (no
  uses remain).

- [ ] **Step 1: The resolution program**

`resolve()` requires `beat == Beat::Resolve` (`WrongBeat` otherwise). Then, in
this order:

1. **Resolve plays** in `order_key` order (the vec is already sorted). Skip
   any play whose source is `Eliminated` by the time its turn comes (7.6).
   Determine subjects per D2 from `card.targets` / `play.target`; a
   `targets == "one"` play whose target is `Eliminated` **fizzles** — no
   effect, pulls stay spent, still occupied its slot (7.5). Apply the D8
   mapping per `CardKind`:
   - Atk: `apply_damage(subject, cost * DMG_PER_COST)` — shields first in
     effect-creation order (consume `magnitude`, remove at 0), remainder off
     HP, `hp = max(0, hp - rest)` (D11). HP hits 0 → **eliminate now**:
     `status = Eliminated`, move `hand` (and any stray `armed`) to
     `discards`, remove effects whose `subject` is this seat (D10).
   - Buff: `hp += cost * HEAL_PER_COST` per subject — no ceiling (TBD-3).
   - Curse: queue `Effect { source_play: play.order_key, subject, op:
     EffectOp::Dot, magnitude: cost * DOT_PER_COST, expires_round:
     self.round + CURSE_ROUNDS }` per subject — queued, appended in step 3,
     so a fresh curse never ticks in its own round.
   - Util / Reaction: nothing (D8).
   Every play's card ends in `discards` — resolved, fizzled and skipped alike
   (8.4, and §14's "the queue empties every round"; one shared vec, per-deck
   filtering happens at reshuffle).
2. **Tick dots** (10.4: creation order): for each existing `Effect` with
   `op == Dot` whose subject is Alive, `apply_damage(subject, magnitude)` —
   same shield/elimination path as step 1.
3. **Append queued effects** with the no-stack rule (TBD-8, D10): a queued
   effect replaces any existing effect with the same `(op, subject)`.
4. **Expire:** remove every effect with `expires_round <= self.round`.
5. **Soft cap** (8.2, D12): any Alive player with `hand.len() > HAND_SOFT_CAP`
   discards from the **end** of the hand down to 12, cards to `discards`.
6. **Bump `seq`.** If `outcome().is_some()`, **stop** — beat stays `Resolve`,
   the final tableau (D16).
7. **Rollover** otherwise: `first_seat = (first_seat + 1) % players.len()`
   (D13); `round += 1`; `beat = Beat::Draw`; for every player `locked =
   false`, `drawing = false`, `draws_this_round = 0`; **reshuffle** (8.4 /
   §12): for each deck whose count is 0, drain that deck's cards from
   `discards` and add their number back to the count.

`apply_damage` is a private helper; write it once, both call sites use it.

- [ ] **Step 2: Kill `NotImplemented`**

All five transitions are now real. Delete the `NotImplemented` variant —
Task 1's `_` arm in `lc_routes.rs` already absorbs the removal, and no other
reference exists. Update the module doc comment (`last_call.rs:1-9`): the
"slice 3 fills in the bodies" paragraph now describes the implemented loop.

- [ ] **Step 3: Tests**

```rust
#[test]
fn test_resolve_applies_damage_and_rolls_over() {
    let mut st = locked_table();      // alice 3 pulls → bob; bob 3 → alice
    st.advance_beat().unwrap();       // Reveal
    st.advance_beat().unwrap();       // Resolve
    st.resolve().unwrap();
    assert_eq!(st.players[1].hp, 9);  // 15 - 2 (beer-01) - 4 (beer-02)
    assert_eq!(st.players[0].hp, 9);  // 15 - 6 (cider-04)
    assert!(st.plays.is_empty());     // the queue empties every round (§14)
    assert_eq!(st.discards.len(), 3);
    assert_eq!(st.round, 2);
    assert_eq!(st.beat, Beat::Draw);
    assert_eq!(st.first_seat, 1);     // rotated (D13)
    assert!(st.players.iter().all(|p| !p.locked && p.draws_this_round == 0));
    assert_eq!(st.outcome(), None);
}

#[test]
fn test_resolve_wrong_beat() {
    let mut st = seated();
    assert_eq!(st.resolve(), Err(LcError::WrongBeat));
}

#[test]
fn test_heal_has_no_ceiling() { // TBD-3
    let mut st = at_lock();
    st.arm(3, "soft-01").unwrap(); // Buff, cost 1, targets "one"
    st.set_target(3, "soft-01", Some(2)).unwrap(); // cara heals herself
    st.lock_in(3).unwrap();
    st.advance_beat().unwrap();
    st.advance_beat().unwrap();
    st.resolve().unwrap();
    assert_eq!(st.players[2].hp, 17); // 15 + 1×HEAL_PER_COST, past start
}

#[test]
fn test_elimination_is_immediate_and_removes_unresolved_plays() { // 7.6, 7.5
    let mut st = at_lock();
    st.players[1].hp = 4;
    // alice arms beer-02 FIRST (4 dmg) then beer-01 (2 dmg), both → bob.
    st.arm(1, "beer-02").unwrap();
    st.set_target(1, "beer-02", Some(1)).unwrap();
    st.arm(1, "beer-01").unwrap();
    st.set_target(1, "beer-01", Some(1)).unwrap();
    st.lock_in(1).unwrap();                    // 3 pulls
    st.arm(2, "cider-04").unwrap();            // bob answers, 3 pulls
    st.set_target(2, "cider-04", Some(0)).unwrap();
    st.lock_in(2).unwrap();
    st.advance_beat().unwrap();
    st.advance_beat().unwrap();
    let bob_hand = st.players[1].hand.len();   // 3 after arming cider-04
    st.resolve().unwrap();
    // Tie at 3 pulls, alice's seat leads: beer-02 lands, bob hits 0 —
    assert_eq!(st.players[1].hp, 0);           // clamped, not negative
    assert_eq!(st.players[1].status, Status::Eliminated);
    // — bob's cider-04 never resolves (alice untouched), his hand discards,
    // and alice's second play fizzles on a dead target with pulls kept:
    assert_eq!(st.players[0].hp, 15);
    assert!(st.players[1].hand.is_empty());    // ghosts hold no cards (9.2)
    assert_eq!(st.players[0].vessels[0].pulls_left, 5); // 7.5: no refund
    // beer-01 + beer-02 + cider-04 + bob's 3 hand cards:
    assert_eq!(st.discards.len(), 6);
    assert_eq!(st.outcome(), None);            // cara still stands
}

#[test]
fn test_last_player_standing_freezes_the_table() { // 9.3, D16
    let mut st = LastCallState::new(vec![(1, "alice".into()), (2, "bob".into())], 42);
    st.set_vessel(1, Deck::Liquor, "shot").unwrap();
    st.players[1].hp = 4;
    st.beat = Beat::Lock;
    st.arm(1, "liquor-02").unwrap(); // Atk cost 3 → 6 dmg
    st.set_target(1, "liquor-02", Some(1)).unwrap();
    st.lock_in(1).unwrap();
    st.advance_beat().unwrap();
    st.advance_beat().unwrap();
    st.resolve().unwrap();
    assert_eq!(st.outcome(), Some(LcOutcome::Winner(0)));
    assert_eq!(st.public_view().outcome, Some(LcOutcome::Winner(0)));
    assert_eq!(st.beat, Beat::Resolve); // frozen final tableau, no rollover
    assert_eq!(st.round, 1);
}

#[test]
fn test_curse_ticks_after_its_round_then_expires() { // D8, D10
    let mut st = at_lock();
    st.arm(2, "cider-01").unwrap(); // Curse cost 1 → Dot mag 1, 2 rounds
    st.set_target(2, "cider-01", Some(0)).unwrap();
    st.lock_in(2).unwrap();
    st.advance_beat().unwrap();
    st.advance_beat().unwrap();
    st.resolve().unwrap();                       // round 1: created, no tick
    assert_eq!(st.players[0].hp, 15);
    assert_eq!(st.effects.len(), 1);
    assert_eq!(st.effects[0].expires_round, 3);  // 1 + CURSE_ROUNDS
    for expected_hp in [14, 13] {                // ticks in rounds 2 and 3
        for _ in 0..5 { st.advance_beat().unwrap(); }
        st.resolve().unwrap();
        assert_eq!(st.players[0].hp, expected_hp);
    }
    assert!(st.effects.is_empty());              // expired after round 3
}

#[test]
fn test_effects_replace_not_stack() { // TBD-8, D10
    let mut st = at_lock();
    st.effects.push(Effect {
        source_play: 0, subject: 0, op: EffectOp::Dot,
        magnitude: 2, expires_round: 9,
    });
    st.arm(2, "cider-01").unwrap();
    st.set_target(2, "cider-01", Some(0)).unwrap();
    st.lock_in(2).unwrap();
    st.advance_beat().unwrap();
    st.advance_beat().unwrap();
    st.resolve().unwrap();
    // The old dot ticked once (15-2), then the new curse replaced it:
    assert_eq!(st.players[0].hp, 13);
    assert_eq!(st.effects.len(), 1);
    assert_eq!((st.effects[0].magnitude, st.effects[0].expires_round), (1, 3));
}

#[test]
fn test_shields_absorb_before_hp() {
    let mut st = at_lock();
    st.effects.push(Effect {
        source_play: 0, subject: 1, op: EffectOp::Shield,
        magnitude: 3, expires_round: 9,
    });
    st.arm(1, "beer-02").unwrap(); // 4 dmg → bob
    st.set_target(1, "beer-02", Some(1)).unwrap();
    st.lock_in(1).unwrap();
    st.advance_beat().unwrap();
    st.advance_beat().unwrap();
    st.resolve().unwrap();
    assert_eq!(st.players[1].hp, 14);  // 3 absorbed, 1 through
    assert!(st.effects.is_empty());    // shield consumed and removed
}

#[test]
fn test_soft_cap_discards_newest_first() { // TBD-2, D12
    let mut st = at_lock();
    st.advance_beat().unwrap(); // Reveal (nobody locked, nothing staged)
    st.advance_beat().unwrap(); // Resolve
    st.players[1].hand =
        std::iter::repeat_n(crate::lc_cards::deck_cards(Deck::Cider), 4)
            .flatten()
            .collect(); // 16
    st.resolve().unwrap();
    assert_eq!(st.players[1].hand.len(), HAND_SOFT_CAP);
    assert_eq!(st.discards.len(), 4);
}

#[test]
fn test_rollover_reshuffles_an_empty_shoe() { // 8.4, §12
    let mut st = at_lock();
    set_deck_count(&mut st, Deck::Beer, 0);
    st.discards = crate::lc_cards::deck_cards(Deck::Beer)[..3].to_vec();
    st.discards.push(crate::lc_cards::card_by_id("cider-01").unwrap());
    st.advance_beat().unwrap();
    st.advance_beat().unwrap();
    st.resolve().unwrap();
    assert_eq!(deck_count(&st, Deck::Beer), 3);
    assert_eq!(st.discards.len(), 1); // the cider card stays put
}
```

- [ ] **Step 4: Commit**

```bash
git add drinkinggame/src/last_call.rs
git commit -m "feat(lastcall): resolve — ordered damage, elimination, effects, soft cap, round rollover; NotImplemented dies"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

## Before the plan is done

- Every task is Class B and every acceptance is `./scripts/verify.sh` — no
  reviewer per task; **one whole-plan review of the branch diff at the end**,
  on the most capable model, per `plan-economics` §4.
- No `cargo sqlx prepare` (no migration; `drinkinggame` is runtime-checked).
- Interfaces line up: Task 3's `payment_plan` is the charge Task 4 applies;
  Task 4's `order_key`-sorted `plays` is the queue Task 5 consumes; Task 1's
  `outcome()` is what Task 5 freezes on and what `PublicView` projects.
- Every spec requirement maps to a task: §3.4.1 → Task 3 (mandatory test);
  the `from_json` cap → Task 1; DDv2 §4 → Task 2; §5–6 → Tasks 3–4; §7–9 →
  Task 5; TBD-1..8 → named constants or recorded decisions.
- The five stub signatures are implemented unrenamed; `NotImplemented` is gone
  by Task 5; `drinkinggame` stays clippy-clean and the distinct-warning count
  stays 17.
- Every DDv2 gap got a concrete rule recorded under "Proposed design
  decisions" (D1–D18) for user review after execution.
