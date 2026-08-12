# Last Call — Plan G: pacts (content system 1)

> **For agentic workers:** REQUIRED SUB-SKILLS: `plan-economics` (this repo's
> task classes, review policy and plan sizing) then
> superpowers:subagent-driven-development to execute task-by-task.

**Goal:** Build pacts — the first content system, DDv2 §10.3's "cheapest system
with the biggest effect on beat 3": secret two-player alliances negotiated
during Diplomacy, with a shared last-two-standing win and a public, named cost
for betrayal.

**Architecture:** Pacts are a pure state-machine extension in `last_call.rs`
following Plan D's patterns — all pact state lives in new top-level
`LastCallState` fields (container `#[serde(default)]` keeps old blobs loading;
`LcPlayer` stays nested-strict) that `public_view()` never reads, except the one
deliberately public record: betrayals. Offer/accept/decline ride Plan E's route
skeleton under the room `RoomLocks` guard and publish `LcTick` only (they change
nothing any public surface renders); the per-viewer pact UI rides the existing
private hand fragment, which takes no player identifier. No new SSE event, no
migration, no RNG.

**Slice:** When this plan is done, Diplomacy is a real beat: players propose,
accept and decline pacts from their phones in secret; a pact silently rewires
the endgame (`LcOutcome::Pact` — the last two standing win together if pacted);
knifing your partner breaks the pact on the big screen by name and bars you from
pacts for the rest of the game. Tabs, events, reactions and ghosts remain the
later content systems; the designed end-of-game screen (and its pact-recap
reveal) stays Plan J.

**Execution order — binding:** This plan runs **after Plan E** (order so far:
C → D → F → E → G). It consumes Plan D's engine, Plan E's route/publish/JS
machinery and Plan C's private-fragment plumbing. **Plans C–F were unexecuted
when this plan was written** — every consumed interface below is copied from
their Produces blocks, and the executor must reconcile against the tree if a
review changed a name.

**Ledger:** `.superpowers/sdd/2026-08-11-last-call-plan-g-pacts/progress.md`
(gitignored).

---

## Proposed design decisions — awaiting user review

DDv2 §10.3 is two sentences and DDv1 §6.3 is a sketch; the recorded gap (spec
§9) is that pacts have **no designed win condition**. Everything below is
proposed, grounded in bundle text where any exists, and implemented as written;
the user design-reviews this list after execution. Every number is a named
constant a playtest can move.

1. **G1 — Pacts are negotiated during Diplomacy, not silently assigned at
   setup. This DIVERGES from DDv2 §10.3** ("Two players are silently paired at
   setup and dealt matching tabs. Neither is told who the other is") **and is
   the headline decision to review.** Why: §10.3's mechanism is literally "dealt
   matching tabs" — it is built ON the tabs system (§10.2), which is hollow and
   a later slice; silent pairing conscripts two players into an alliance
   neither chose; and DDv1's entire "free wink" apparatus exists only to patch
   the you-don't-know-your-partner problem, which negotiation deletes outright.
   What survives from the bundle: the secrecy ("pacts are secret by
   definition", spec §3.3), the beat-3 payoff (DDv2: "the biggest effect on
   beat 3" — Diplomacy talk becomes cover for pact traffic), and DDv1's win
   condition and safety-valve spirit (G2, G6, G7).
2. **G2 — The win condition is DDv1 §6.3's, adopted verbatim:** *"If you and
   your partner are the last two standing, you both win."* `outcome()` returns
   the new `LcOutcome::Pact(a, b)` when exactly two players are Alive and they
   share a pact; the freeze mechanics are identical to `Winner` (D16's final
   tableau). This closes the spec-§9 recorded gap.
3. **G3 — A pact does nothing else mechanically.** DDv1: *"it's two tabs and a
   shared win check."* No buffs, no cost changes, no drink changes — the
   cheapest system stays cheap, and §10.1's one-extra-pull ceiling for events
   is honoured by analogy (a pact never touches drinking at all).
4. **G4 — Betrayal = a resolved single-target hostile play on your partner:**
   `targets == "one"`, target is the partner's seat, and the card's fx op is
   `Damage`, `Dot` or `PullDrain`. AOE splash is **not** betrayal (the self-hit
   and table-hit are the card's priced-in joke, F9); heals and shields on the
   partner are obviously not. Fizzled plays break nothing.
5. **G5 — Betrayal's cost is social, and it is loud:** the break is recorded
   publicly by name (`PactBreak`, projected into `PublicView` — the one pact
   field the projection may read) and rendered on the big screen for the round
   it happens; the betrayer is **barred from pacts for the rest of the game**.
   No HP or pull penalty — per G3 the system never touches the drink economy.
6. **G6 — One pact per player at a time; no room-wide cap.** DDv1's valve
   ("Only ever one pact per game, and only at 5+ players. Two pacts in a
   six-player game is four people not playing the game") guarded *conscripted*
   pacts; a chosen alliance is players playing the game harder. The DDv1
   counterpoint is quoted here so the user can overrule with one number.
7. **G7 — Offers require `PACT_MIN_ALIVE` (4) Alive players.** With 2 alive an
   offer is an instant-win button; with 3 it is a formalized 2-v-1. DDv1's
   "assume a crowd" carried into the negotiated model. Existing pacts still pay
   off below the threshold — that is the endgame (G2).
8. **G8 — Offers are beat-scoped:** made only during Diplomacy, cleared
   unanswered at the Diplomacy→Lock edge (expiry is the decline nobody had to
   press). One outgoing offer at a time — re-offering the same seat is
   idempotent (Ok, no seq bump), offering a different seat retargets. Mutual
   pending offers form the pact directly (two people proposing to each other
   should get a pact, not an error).
9. **G9 — Pact lifetime:** until betrayal, until a partner's elimination
   (silent dissolve, no break record — the survivor may pact again in a later
   Diplomacy), or game end. Never expires on its own; `formed_round` is kept
   for display.
10. **G10 — Nothing is public until a betrayal or the win.** The brief permits
    a public "in talks"/pact-count tick if justifiable — declined: any public
    correlate of offer traffic converts Diplomacy body language into a UI read,
    and DDv2 6.3's only-a-lock-tick precedent points the same way. The
    broadcast surface carries pact information exactly twice: a `PactBreak`
    and a `Pact` outcome.
11. **G11 — An offer to an unavailable target quietly goes nowhere.** Offers to
    already-pacted (or barred) players are accepted and recorded but can never
    be accepted by them — indistinguishable, to the offeror, from being
    ignored. The alternative (refusing with an error, or hiding pacted players
    from the propose list) turns the offer button into a pact detector; the
    propose list therefore excludes only *public* knowledge (yourself,
    eliminated seats, publicly-barred betrayers) and includes secretly-pacted
    players as no-op targets.
12. **G12 — Diplomacy stays timer-only (Plan E's E3 stands).** No
    "everyone's done" early exit: an exit conditioned on pact traffic would
    itself leak pact traffic (G10).
13. **G13 — All pact state lives on `LastCallState` top level** (`pacts`,
    `pact_offers`, `pact_barred`, `pact_breaks`) — the serde version-skew rule
    (container-level default; nested structs strict) forbids new `LcPlayer`
    fields, and top-level fields are also what keeps the `public_view()`
    discipline auditable in one place.
14. **G14 — DDv1's "free wink" is dropped** (its problem — finding an unknown
    partner — no longer exists, per G1) **and DDv1's end-of-game reveal of
    unsettled pacts is deferred to Plan J's designed end screen.** This plan
    reveals a pact only through its win or its break.

---

## Global Constraints

Every task's requirements implicitly include this section.

### Spec bindings — all still in force

- **Spec §3.3 / §6.1:** private state is fetched per viewer, never broadcast.
  The pact UI rides `GET /room/{code}/lastcall/hand`, which keeps taking **no
  player identifier**; the three new mutating routes name no player either —
  the actor is the session cookie (`PlayerSession`), offer targets are *seats*
  validated by the engine (Plan E's rule, verbatim).
- **The §3.4.1 pattern applied to pacts:** `pacts`, `pact_offers` and
  `pact_barred` are fields `public_view()` never reads. `pact_breaks` is the
  single exception, by design (G5) — betrayal is public. Task 1 owns the unit
  test; Task 4 owns the MANDATORY end-to-end test (a pact between A and B is
  absent from `public_view()` output, from every broadcast frame, and from C's
  private fragment).
- **Publish discipline (Plan E's E5):** offer/accept/decline publish `LcTick`
  only via `persist_and_tick_lc` — they change nothing any public surface
  renders (G10), but both parties' phones need the private re-fetch signal.
  Publish order `room` → `lcpublic` → `lctick` and the await-free
  `broadcast_lc`/`persist_and_tick_lc` are untouched. **No new SSE event.**
- **seq discipline:** every successful mutating transition bumps `seq` exactly
  once; failed calls and idempotent replays bump nothing (Plan D's rule).
- SSE tests **filter frames by event name/content via `read_sse_until`, never
  index positionally**.
- Renderers emit deck class names and tokens, never hex; new builders join the
  no-hex sweep. New buttons are **not** `lc-btn-drink` — amber is reserved for
  drinking-adjacent primaries (E7), and a pact costs no drink (G3).
- One `@media (prefers-reduced-motion: reduce)` block in `lastcall.css`; this
  plan adds nothing animated, so it must not touch that block at all.
- Ring of Fire and 3 Man are untouched. `test_rof_sse_snapshot_has_no_lcpublic`,
  `test_tm_sse_snapshot_includes_tm_panels` and
  `test_sse_snapshot_has_all_stateful_kinds` stay green after every task.
- Keep `drinkinggame` clippy-clean; the pre-existing distinct-warning count
  (17, all `drawingportfolio`) must not grow.

### Consumed interfaces (from unexecuted plans' Produces blocks — reconcile against the tree)

```rust
// Plan D (last_call.rs):
pub fn advance_beat(&mut self) -> Result<(), LcError>;   // G touches its Diplomacy→Lock edge
pub fn resolve(&mut self) -> Result<(), LcError>;        // G inserts the betrayal check + dissolve sweep
pub fn outcome(&self) -> Option<LcOutcome>;              // G inserts the pact-win arm
pub enum LcOutcome { Winner(usize), Draw }               // G adds Pact(usize, usize)
pub enum LcError { NotSeated, BadHandicap, WrongBeat, NotAlive, AlreadyLocked,
    UnknownCard, NotPlayable, CantAfford(String), NeedsTarget(String),
    BadTarget, BadDraw, MustResolve }                    // G adds PactBlocked, NoOffer
// Plan F (lc_cards.rs):
pub fn card_fx(id: &str) -> Option<FxDef>;               // FxDef { op, magnitude, rounds }
pub enum EffectOp { Damage, Heal, Shield, Dot, PullDrain }
// Plan E (lc_routes.rs):
pub(crate) fn map_lc(e: LcError) -> axum::response::Response;
pub(crate) async fn persist_and_tick_lc(state: &GameState, ctx: &LcCtx);
fn hand_pane_html(base_path: &str, code: &str, st: &LastCallState, player_id: i64) -> String;
fn targets_section_html(st: &LastCallState, seat: usize) -> String; // the section pattern G copies
// Plan E (assets/lc_loop.js):
//   post(action, body); note(text); the ONE delegated [data-lc-post] click
//   listener on document.body (G extends its body construction).
// Plan E (lc_render.rs): the LcOutcome match inside lc_screen_panel's
//   game-over centre ("{NAME} OUTLASTS THE TABLE" / "EVERYBODY'S OUT") — G's
//   new variant makes it non-exhaustive, which is the compile-forced hook.
// Since slice 1: lc_lock, load_lc, LcCtx, persist_and_broadcast_lc,
//   html_escape, ring_fixture()'s PublicView literal, read_sse_until,
//   the set_game_state seeding pattern (http.rs:2213).
```

### New constants (in `last_call.rs`, playtest-movable)

```rust
/// G7: pact offers need this many Alive players. Existing pacts still pay off
/// below it — that is the endgame (G2).
pub const PACT_MIN_ALIVE: usize = 4;
```

### Verification

**Verification for every task:** `./scripts/verify.sh` — all green, output
quoted in the report. Never bare `cargo test` (it skips `drinkinggame`
entirely).

**Baseline before Task 1:** whatever Plan E's ledger records at its close. The
pre-C figure was 371 tests; the invariants are *verify green* and *17 distinct
clippy warnings, all `drawingportfolio`, `drinkinggame` clean* — not a fixed
test count. Read the number from
`.superpowers/sdd/2026-08-11-last-call-plan-e-loop-wiring/progress.md` and
record it in this plan's ledger before starting.

**Browser checkpoint: one**, after Task 4, before the whole-plan review. **No
`cargo sqlx prepare`** (no migration; `drinkinggame` is runtime-checked).

---

### Task 1: Engine — pact vocabulary and the Diplomacy transitions

**Class:** B (logic, tests specified below)

**Why this class:** pure transitions over new fields with every case and
expected value written here; the projection-absence test is a JSON assertion,
the tests are the spec.

**Files:**
- Modify: `drinkinggame/src/last_call.rs`
- Test: `drinkinggame/src/last_call.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: `Beat::Diplomacy`, `Status`, `seat_of`, Plan D's `advance_beat`
  (its Diplomacy→Lock edge), the seq-discipline precedent (`add_player`).
- Produces (Tasks 2–4 build against these — exact):

```rust
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pact { pub a: usize, pub b: usize, pub formed_round: u32 } // a < b always
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PactOffer { pub from: usize, pub to: usize }
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PactBreak { pub betrayer: usize, pub betrayed: usize, pub round: u32 }

// LastCallState gains (all four covered by the container #[serde(default)]):
//   pub pacts: Vec<Pact>,          // NEVER projected (G13; the §3.4.1 pattern)
//   pub pact_offers: Vec<PactOffer>, // NEVER projected; cleared at Diplomacy→Lock
//   pub pact_barred: Vec<usize>,   // NEVER projected as a field — derivable from
//                                  // pact_breaks, which are public (G5)
//   pub pact_breaks: Vec<PactBreak>, // the ONE pact field public_view may read (Task 2)

// LcError gains:
//   PactBlocked,  // offer/accept refused: you are pacted or barred, or < PACT_MIN_ALIVE alive
//   NoOffer,      // accept/decline names no pending offer from that seat

pub fn offer_pact(&mut self, player_id: i64, target_seat: usize) -> Result<(), LcError>;
pub fn accept_pact(&mut self, player_id: i64, from_seat: usize) -> Result<(), LcError>;
pub fn decline_pact(&mut self, player_id: i64, from_seat: usize) -> Result<(), LcError>;
/// The seat's current partner, if any. Used by resolve() (Task 2) and the
/// private section renderer (Task 3). Reads `pacts` — callable only from
/// engine internals and per-viewer code, never from a public renderer.
pub fn pact_partner(&self, seat: usize) -> Option<usize>;
pub const PACT_MIN_ALIVE: usize = 4;
```

- [ ] **Step 1: types, fields, constant**

Add the three structs, the four fields (update `LastCallState::new`'s literal
with empty vecs), the two `LcError` variants and `PACT_MIN_ALIVE`. Doc-comment
the fields with the projection rule above — the comment is load-bearing: it is
what a future reviewer greps for before touching `public_view()`.

- [ ] **Step 2: `offer_pact`**

Guard order: `NotSeated` → `NotAlive` → `WrongBeat` (must be
`Beat::Diplomacy`) → `BadTarget` (target is self, out of range, or not
`Alive`) → `PactBlocked` (the *offeror* is pacted or barred, or fewer than
`PACT_MIN_ALIVE` players are Alive — all three are facts the offeror already
knows, so refusing leaks nothing; target-side unavailability deliberately does
NOT refuse, per G11). Then:

- **Mutual offer** (G8): if `pact_offers` contains the reverse offer
  `{ from: target_seat, to: my_seat }`, form the pact immediately — push
  `Pact { a: min, b: max, formed_round: self.round }`, remove **only** the
  offers between the two seats (both directions; third-party offers stay
  pending so their owners' WAITING lines cannot become a pact detector — G11),
  `seq += 1`. (Invariant making this safe: a reverse offer implies the target
  was unpacted and unbarred when they offered, and formation/betrayal within
  the beat clears/cannot-create their outgoing offers.)
- **Idempotent repeat**: an identical pending offer already exists → `Ok(())`,
  no bump (the `add_player` precedent).
- **Otherwise**: remove any other outgoing offer from this seat (one at a
  time, retarget replaces — G8), push `PactOffer { from: my_seat, to:
  target_seat }`, `seq += 1`. This records offers to secretly-pacted targets
  too — they are no-ops that expire (G11).

- [ ] **Step 3: `accept_pact` and `decline_pact`**

`accept_pact` guards: `NotSeated` → `NotAlive` → `WrongBeat` → `PactBlocked`
(the accepter is pacted or barred — such players are never *shown* offers, so
this is route-level defence) → `NoOffer` (no pending
`PactOffer { from: from_seat, to: my_seat }`). Success: push
`Pact { a: min, b: max, formed_round: self.round }`, remove the offers between
the two seats (both directions only, as in Step 2), `seq += 1`.

`decline_pact` guards: same chain including `PactBlocked`, then `NoOffer`.
Success: remove that one offer, `seq += 1`. (The offeror's WAITING line
reverting to a propose button is the answer they are owed — a decline is a
signal from someone who saw the offer, which only an available player can be.)

`pact_partner`: scan `pacts` for the seat, return the other end.

- [ ] **Step 4: offer expiry**

In `advance_beat`'s per-edge work, the Diplomacy→Lock edge (Plan D Task 4
wrote it as "no extra work") gains:

```rust
// G8: offers are beat-scoped. Clearing here is the decline nobody had to
// press — and it is why no offer can ever dangle across an elimination
// (eliminations happen at Resolve, offers never survive past Diplomacy).
self.pact_offers.clear();
```

No extra seq bump — `advance_beat` already bumps.

- [ ] **Step 5: tests**

Shared fixture at the top of the tests module:

```rust
/// alice(1)/Beer, bob(2)/Cider, cara(3)/Soft, dave(4)/Liquor — seats 0-3.
/// Vessels registered at Draw, then moved to Diplomacy.
fn at_diplomacy() -> LastCallState {
    let mut st = LastCallState::new(
        vec![(1, "alice".into()), (2, "bob".into()),
             (3, "cara".into()), (4, "dave".into())],
        42,
    );
    st.set_vessel(1, Deck::Beer, "can").unwrap();
    st.set_vessel(2, Deck::Cider, "bottle").unwrap();
    st.set_vessel(3, Deck::Soft, "glass").unwrap();
    st.set_vessel(4, Deck::Liquor, "shot").unwrap();
    st.beat = Beat::Diplomacy;
    st
}
```

```rust
#[test]
fn test_offer_and_accept_form_a_pact() {
    let mut st = at_diplomacy();
    let before = st.seq;
    st.offer_pact(1, 1).unwrap(); // alice (seat 0) -> bob (seat 1)
    assert_eq!(st.pact_offers, vec![PactOffer { from: 0, to: 1 }]);
    assert_eq!(st.seq, before + 1);
    st.accept_pact(2, 0).unwrap(); // bob accepts alice's offer
    assert_eq!(st.pacts, vec![Pact { a: 0, b: 1, formed_round: 1 }]);
    assert!(st.pact_offers.is_empty());
    assert_eq!(st.pact_partner(0), Some(1));
    assert_eq!(st.pact_partner(1), Some(0));
    assert_eq!(st.pact_partner(2), None);
    assert_eq!(st.seq, before + 2);
}

#[test]
fn test_offer_guard_order() {
    let mut st = at_diplomacy();
    assert_eq!(st.offer_pact(999, 1), Err(LcError::NotSeated));
    assert_eq!(st.offer_pact(1, 0), Err(LcError::BadTarget)); // self
    assert_eq!(st.offer_pact(1, 9), Err(LcError::BadTarget)); // no such seat
    st.players[2].status = Status::Eliminated;
    assert_eq!(st.offer_pact(1, 2), Err(LcError::BadTarget)); // dead target
    // 3 alive < PACT_MIN_ALIVE — even a valid target is refused (G7):
    assert_eq!(st.offer_pact(1, 1), Err(LcError::PactBlocked));
    st.players[2].status = Status::Alive;
    st.players[0].status = Status::Eliminated;
    assert_eq!(st.offer_pact(1, 1), Err(LcError::NotAlive));
    st.players[0].status = Status::Alive;
    st.beat = Beat::Lock;
    assert_eq!(st.offer_pact(1, 1), Err(LcError::WrongBeat));
}

#[test]
fn test_one_outgoing_offer_retargets_and_repeats_are_free() {
    let mut st = at_diplomacy();
    st.offer_pact(1, 1).unwrap();
    let seq = st.seq;
    st.offer_pact(1, 1).unwrap(); // identical repeat: Ok, no bump (G8)
    assert_eq!((st.seq, st.pact_offers.len()), (seq, 1));
    st.offer_pact(1, 2).unwrap(); // retarget replaces (G8)
    assert_eq!(st.pact_offers, vec![PactOffer { from: 0, to: 2 }]);
    assert_eq!(st.seq, seq + 1);
}

#[test]
fn test_mutual_offers_form_the_pact_directly() { // G8
    let mut st = at_diplomacy();
    st.offer_pact(1, 1).unwrap();
    st.offer_pact(2, 0).unwrap(); // bob offers alice back
    assert_eq!(st.pacts, vec![Pact { a: 0, b: 1, formed_round: 1 }]);
    assert!(st.pact_offers.is_empty());
}

#[test]
fn test_offers_to_the_unavailable_go_quietly_nowhere() { // G11
    let mut st = at_diplomacy();
    st.offer_pact(1, 1).unwrap();
    st.accept_pact(2, 0).unwrap(); // alice-bob pacted
    // cara offers pacted alice: recorded, not refused — no pact detector.
    st.offer_pact(3, 0).unwrap();
    assert_eq!(st.pact_offers, vec![PactOffer { from: 2, to: 0 }]);
    // alice (pacted) cannot accept it:
    assert_eq!(st.accept_pact(1, 2), Err(LcError::PactBlocked));
    // pacted alice cannot offer either:
    assert_eq!(st.offer_pact(1, 3), Err(LcError::PactBlocked));
    // barred players are out of the market on the offering side too:
    st.pact_barred.push(3); // dave
    assert_eq!(st.offer_pact(4, 2), Err(LcError::PactBlocked));
    // ...but can still be offered to (their bar is public; the offer no-ops):
    st.offer_pact(3, 3).unwrap(); // cara retargets dave
    assert_eq!(st.accept_pact(4, 2), Err(LcError::PactBlocked));
}

#[test]
fn test_accept_and_decline_need_a_real_offer() {
    let mut st = at_diplomacy();
    assert_eq!(st.accept_pact(2, 0), Err(LcError::NoOffer));
    st.offer_pact(1, 1).unwrap();
    let seq = st.seq;
    st.decline_pact(2, 0).unwrap();
    assert!(st.pact_offers.is_empty());
    assert_eq!(st.seq, seq + 1);
    assert_eq!(st.decline_pact(2, 0), Err(LcError::NoOffer)); // already gone
    assert!(st.pacts.is_empty());
}

#[test]
fn test_offers_expire_when_diplomacy_ends() { // G8
    let mut st = at_diplomacy();
    st.offer_pact(1, 1).unwrap();
    st.offer_pact(3, 3).unwrap();
    st.advance_beat().unwrap(); // Diplomacy -> Lock
    assert_eq!(st.beat, Beat::Lock);
    assert!(st.pact_offers.is_empty());
}

/// The §3.4.1 pattern applied to pacts (Global Constraints): nothing
/// pact-shaped reaches the projection. Task 2 adds the deliberate exception
/// (pact_breaks) and NARROWS this assertion to the named private keys —
/// planned there, not discovered.
#[test]
fn test_pacts_and_offers_never_reach_the_public_view() {
    let mut st = at_diplomacy();
    st.offer_pact(1, 1).unwrap();
    st.accept_pact(2, 0).unwrap();      // a formed pact
    st.offer_pact(3, 3).unwrap();       // and a pending offer
    st.pact_barred.push(3);
    for beat in [Beat::Draw, Beat::Deal, Beat::Diplomacy,
                 Beat::Lock, Beat::Reveal, Beat::Resolve] {
        st.beat = beat;
        let json = serde_json::to_string(&st.public_view()).unwrap();
        assert!(!json.contains("pact"), "beat={beat:?}: {json}");
    }
}
```

Also assert round-tripping: extend the existing `test_serde_round_trip` state
with a pact, an offer, a barred seat and a break record so the four fields are
pinned into the blob format.

- [ ] **Step 6: Commit**

```bash
git add drinkinggame/src/last_call.rs
git commit -m "feat(lastcall): pact offers — Diplomacy-scoped, secret, mutual-consent; nothing reaches public_view"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

### Task 2: Engine — betrayal, dissolution, and the pact win

**Class:** B (logic, tests specified below)

**Why this class:** pure additions to `resolve()` and `outcome()` with every
rule pinned by a test and expected values written here; no concurrency, auth
or broadcast.

**Files:**
- Modify: `drinkinggame/src/last_call.rs`
- Modify: `drinkinggame/src/lc_render.rs` (the compile-forced `LcOutcome`
  match arm in `lc_screen_panel`'s game-over centre; `ring_fixture()`'s
  `PublicView` literal gains `pact_breaks: vec![],`)
- Test: `drinkinggame/src/last_call.rs`, `drinkinggame/src/lc_render.rs`

**Interfaces:**
- Consumes: Task 1's types/fields/`pact_partner`; Plan D's `resolve()`
  structure and elimination path; Plan F's `card_fx`/`EffectOp`; Plan E's
  game-over centre match.
- Produces (exact):

```rust
// LcOutcome gains (serde snake_case -> {"pact":[a,b]}):
//   Pact(usize, usize),   // the two winning seats, a < b
// PublicView gains — the ONE public pact field (G5), projected verbatim:
//   pub pact_breaks: Vec<PactBreak>,
// outcome(): exactly two Alive players sharing a pact -> Some(LcOutcome::Pact(a, b)).
// resolve(): the betrayal check (G4/G5) and the dissolve sweep (G9).
```

- [ ] **Step 1: the betrayal check (G4/G5)**

In `resolve()`'s step-1 play loop, **after** the existing skip
(dead source) and fizzle (dead "one"-target) gates and before the fx is
applied, insert:

```rust
// G4/G5: a resolved single-target hostile play on your partner is betrayal.
// After the fizzle gate on purpose — a play fizzling on an already-dead
// partner breaks nothing; the end-of-resolve sweep dissolves that pact
// silently instead (G9).
if play.card.targets == "one" {
    if let (Some(target), Some(partner)) = (play.target, self.pact_partner(play.source_seat)) {
        let hostile = crate::lc_cards::card_fx(&play.card.id).is_some_and(|f| {
            matches!(f.op, EffectOp::Damage | EffectOp::Dot | EffectOp::PullDrain)
        });
        if target == partner && hostile {
            self.pacts.retain(|p| !(p.a.min(p.b) == play.source_seat.min(target)
                && p.a.max(p.b) == play.source_seat.max(target)));
            self.pact_breaks.push(PactBreak {
                betrayer: play.source_seat,
                betrayed: target,
                round: self.round,
            });
            self.pact_barred.push(play.source_seat); // for the rest of the game (G5)
        }
    }
}
```

(Adapt the retain to the tree's actual `Pact { a, b }` ordering invariant —
`a < b` per Task 1, so the retain simplifies to comparing the sorted pair.)
The break happens whether or not a shield absorbs the hit — the knife was
public. AOE plays (`targets != "one"`) never reach this branch (G4).

- [ ] **Step 2: the dissolve sweep (G9)**

Between `resolve()`'s effect-expiry step and its seq/outcome step, add:

```rust
// G9: a pact whose partner is gone has no win to share. Silent — no break
// record; the survivor may pact again in a later Diplomacy. (Offers need no
// sweep: they never survive past Diplomacy — see advance_beat.)
self.pacts.retain(|p| {
    self.players[p.a].status == Status::Alive
        && self.players[p.b].status == Status::Alive
});
```

A sweep rather than a hook inside the elimination helper, so it cannot depend
on where in the resolution order the death happened.

- [ ] **Step 3: the pact win (G2)**

`LcOutcome` gains `Pact(usize, usize)`. `outcome()` becomes:

```rust
pub fn outcome(&self) -> Option<LcOutcome> {
    if self.players.len() < 2 {
        return None;
    }
    let alive: Vec<usize> = self
        .players
        .iter()
        .filter(|p| p.status == Status::Alive)
        .map(|p| p.seat)
        .collect();
    match alive.as_slice() {
        [] => Some(LcOutcome::Draw),
        [w] => Some(LcOutcome::Winner(*w)),
        // G2 — DDv1 6.3: "If you and your partner are the last two standing,
        // you both win." players is seat-ordered, so a < b holds.
        [a, b] if self.pacts.iter().any(|p| p.a == *a && p.b == *b) => {
            Some(LcOutcome::Pact(*a, *b))
        }
        _ => None,
    }
}
```

The D16 freeze needs no change — `resolve()` and `lc_advance_chain` already
stop on any `Some`.

- [ ] **Step 4: the public projection and the compile-forced render arms**

`PublicView` gains `pub pact_breaks: Vec<PactBreak>,`; `public_view()`
projects it verbatim (`self.pact_breaks.clone()`) with a comment naming G5 —
the one pact field the projection reads. `ring_fixture()`'s literal gains
`pact_breaks: vec![],`.

In `lc_render.rs`, the `LcOutcome` match inside `lc_screen_panel`'s game-over
centre (Plan E Task 5) is now non-exhaustive — add the arm with the final
copy, names uppercased and `html_escape`d, resolved from `view.seats`:

```
LcOutcome::Pact(a, b) -> "{A} & {B} — THE PACT HOLDS"
```

The banner keeps "GAME OVER" and the action bar keeps END GAME unchanged
(both branch on `is_some()`, not the variant).

**Narrow Task 1's projection test as planned there:**
`test_pacts_and_offers_never_reach_the_public_view`'s blanket
`!json.contains("pact")` becomes, with a comment naming this task:

```rust
assert!(!json.contains("\"pacts\""), "beat={beat:?}");
assert!(!json.contains("pact_offers"), "beat={beat:?}");
assert!(!json.contains("pact_barred"), "beat={beat:?}");
assert!(!json.contains("formed_round"), "beat={beat:?}");
// The one public pact field stays empty while every pact is intact (G10):
assert!(view.pact_breaks.is_empty(), "beat={beat:?}");
```

- [ ] **Step 5: tests**

```rust
#[test]
fn test_betrayal_breaks_the_pact_publicly_and_bars_the_betrayer() {
    let mut st = at_diplomacy();
    st.offer_pact(1, 1).unwrap();
    st.accept_pact(2, 0).unwrap(); // alice-bob
    st.beat = Beat::Lock;
    st.arm(1, "beer-01").unwrap(); // Damage 2, targets "one"
    st.set_target(1, "beer-01", Some(1)).unwrap(); // ...at her partner
    st.lock_in(1).unwrap();
    st.advance_beat().unwrap(); // Reveal
    st.advance_beat().unwrap(); // Resolve
    st.resolve().unwrap();
    assert_eq!(st.players[1].hp, 13);
    assert!(st.pacts.is_empty());
    assert_eq!(st.pact_breaks,
        vec![PactBreak { betrayer: 0, betrayed: 1, round: 1 }]);
    assert_eq!(st.pact_barred, vec![0]);
    // The break is the one public trace (G5):
    assert_eq!(st.public_view().pact_breaks, st.pact_breaks);
    // Next Diplomacy: the betrayer is out of the market for good.
    st.beat = Beat::Diplomacy;
    assert_eq!(st.offer_pact(1, 2), Err(LcError::PactBlocked));
    assert_eq!(st.accept_pact(1, 2), Err(LcError::PactBlocked));
}

#[test]
fn test_aoe_splash_and_kindness_are_not_betrayal() { // G4
    // Splash: alice-bob pacted, alice plays beer-05 (aoe, hits bob too).
    let mut st = at_diplomacy();
    st.offer_pact(1, 1).unwrap();
    st.accept_pact(2, 0).unwrap();
    st.beat = Beat::Lock;
    st.arm(1, "beer-05").unwrap(); // Damage 1 to all, incl. bob and alice
    st.lock_in(1).unwrap();
    st.advance_beat().unwrap();
    st.advance_beat().unwrap();
    st.resolve().unwrap();
    assert!(st.players.iter().all(|p| p.hp == 14));
    assert_eq!(st.pacts.len(), 1); // intact
    assert!(st.pact_breaks.is_empty() && st.pact_barred.is_empty());

    // Kindness: cara-bob pacted, cara heals bob ("one"-target, friendly op).
    let mut st = at_diplomacy();
    st.offer_pact(3, 1).unwrap();
    st.accept_pact(2, 2).unwrap();
    st.beat = Beat::Lock;
    st.arm(3, "soft-01").unwrap(); // Heal 2, targets "one"
    st.set_target(3, "soft-01", Some(1)).unwrap();
    st.lock_in(3).unwrap();
    st.advance_beat().unwrap();
    st.advance_beat().unwrap();
    st.resolve().unwrap();
    assert_eq!(st.players[1].hp, 17);
    assert_eq!(st.pacts.len(), 1);
    assert!(st.pact_breaks.is_empty());
}

#[test]
fn test_elimination_dissolves_the_pact_silently() { // G9
    let mut st = at_diplomacy();
    st.offer_pact(1, 1).unwrap();
    st.accept_pact(2, 0).unwrap(); // alice-bob
    st.players[1].hp = 2;
    st.beat = Beat::Lock;
    st.arm(3, "soft-06").unwrap(); // cara (no pact): Damage 2, "one"
    st.set_target(3, "soft-06", Some(1)).unwrap();
    st.lock_in(3).unwrap();
    st.advance_beat().unwrap();
    st.advance_beat().unwrap();
    st.resolve().unwrap();
    assert_eq!(st.players[1].status, Status::Eliminated);
    assert!(st.pacts.is_empty());        // dissolved
    assert!(st.pact_breaks.is_empty());  // silently — no break, nobody barred
    assert!(st.pact_barred.is_empty());
}

#[test]
fn test_the_last_two_standing_share_the_win() { // G2
    let mut st = at_diplomacy();
    st.offer_pact(1, 1).unwrap();
    st.accept_pact(2, 0).unwrap(); // alice-bob
    st.players[2].hp = 1;
    st.players[3].hp = 1;
    st.beat = Beat::Lock;
    st.arm(1, "beer-05").unwrap(); // aoe 1: kills cara and dave, splashes bob
    st.lock_in(1).unwrap();
    st.advance_beat().unwrap();
    st.advance_beat().unwrap();
    st.resolve().unwrap();
    // The splash on bob did not betray (G4), the two deaths dissolved no
    // pact of theirs, and the pair stands:
    assert_eq!(st.outcome(), Some(LcOutcome::Pact(0, 1)));
    assert_eq!(st.public_view().outcome, Some(LcOutcome::Pact(0, 1)));
    assert_eq!(st.beat, Beat::Resolve); // frozen final tableau (D16)
    // Without a pact the same tableau plays on:
    let mut st2 = at_diplomacy();
    st2.players[2].status = Status::Eliminated;
    st2.players[3].status = Status::Eliminated;
    assert_eq!(st2.outcome(), None);
    // And the serde name is pinned:
    assert_eq!(serde_json::to_string(&LcOutcome::Pact(0, 1)).unwrap(),
               r#"{"pact":[0,1]}"#);
}
```

In `lc_render.rs`'s tests, extend Plan E's
`test_game_over_takes_over_banner_and_centre` (or add a sibling test if the
tree's shape prefers): a `PublicView` with `outcome:
Some(LcOutcome::Pact(0, 2))` renders the centre line
`"{S0} & {S2} — THE PACT HOLDS"` with both names taken from
`view.seats[..].name.to_uppercase()`, the banner still says `GAME OVER`, and
`no_hex` holds.

- [ ] **Step 6: Commit**

```bash
git add drinkinggame/src/last_call.rs drinkinggame/src/lc_render.rs
git commit -m "feat(lastcall): betrayal breaks a pact by name and bars the knife; the last two standing win together"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

### Task 3: The pact surfaces — private section, break strip, `data-lc-body`

**Class:** B (logic, tests specified below — string builders with exact
expected copy plus a one-line static-JS extension gated by `node --check`;
the per-viewer render is pinned by unit and http tests, the session gate
itself is untouched)

**Files:**
- Modify: `drinkinggame/src/lc_routes.rs` (`pacts_section_html`;
  `hand_pane_html` inserts it; unit tests in the existing
  `#[cfg(test)] mod tests`)
- Modify: `drinkinggame/src/lc_render.rs` (`lc_screen_panel` gains the break
  strip; tests)
- Modify: `drinkinggame/assets/lc_loop.js` (the `data-lc-body` branch)
- Modify: `drinkinggame/assets/lastcall.css` (one section)
- Test: `drinkinggame/tests/http.rs`

**Interfaces:**
- Consumes: Task 1's fields/`pact_partner`, Task 2's `pact_breaks`
  projection; Plan E's `hand_pane_html`, `targets_section_html` (the section
  pattern), the `[data-lc-post]` delegated click listener in `lc_loop.js`;
  `html_escape`; `PACT_MIN_ALIVE`.
- Produces (Task 4's tests and JS build against these — exact):
  - `fn pacts_section_html(st: &LastCallState, player_id: i64) -> String` in
    `lc_routes.rs` — empty string when there is nothing to show.
  - `hand_pane_html` returns
    `{lc_hand_pane(...)}{targets_section}{pacts_section}{<template data-lc-actions>...}`
    — `#lc-hand` still leads, so the seq gate and stale-drop are untouched.
  - `lc_loop.js`: `[data-lc-post]` buttons may carry `data-lc-body`, a
    pre-encoded form body posted verbatim.
  - DOM contract:

    | Component | Root | Requires | Exposes |
    | --- | --- | --- | --- |
    | Pacts section | `.lc-pacts` | — | `.lc-pact-standing`, `.lc-pact-pending`, `.lc-pact-barred`, `.lc-pact-broken`, `.lc-pact-offer-row`, buttons `[data-lc-post][data-lc-body]` |
    | Break strip | `.lc-pact-break` | — | one per current-round `PactBreak`, big screen only |

> **ERRATUM (2026-08-12, whole-plan review).** Both `round == st.round`
> filters below — Step 1's betrayed notice and Step 2's break strip — are
> **wrong as written and were corrected in the shipped code.** The brief
> assumes `PactBreak.round` and `st.round` are read at the same moment the
> break is recorded; they never are. Task 2's `resolve()` pushes
> `PactBreak { round: self.round }` in its Step 1 (the play loop), then —
> unless that same betrayal also ends the game — bumps `self.round` in its
> own Step 8 rollover before returning. `lc_advance_chain` never persists an
> intermediate state between the two, so the first frame any client can
> fetch already has `st.round` one past the stamp: for every non-terminal
> betrayal (the common case), both filters as written are permanently
> unreachable, and only a game-ending betrayal ever showed — the opposite of
> G5 ("loud, by name"). Found by Task 3's implementer, pinned empirically,
> adjudicated here rather than guessed at inside a single task. Shipped
> semantic: `resolve()` now stamps a non-terminal break with the round it
> rolls over INTO (the round players actually land on when they next fetch),
> not the round it was thrown in; a terminal break keeps the round the game
> froze on (D16 — `beat` never leaves `Resolve`, so that stamp never goes
> stale). Both filters below are unchanged in code — `round == st.round` /
> `round == view.round` are correct once the write side stamps the right
> round — so a betrayal is loud for exactly the one round following it, then
> ages out. Wherever this plan says "current round" for either surface, read
> "the round the break becomes visible in," not "the round it happened in."

- [ ] **Step 1: `pacts_section_html`**

Follows `targets_section_html`'s shape and placement. Returns `""` for a
non-member. Otherwise emits `<section class="lc-pacts"><h2>Pact</h2>…</section>`
containing, in this order (names via `html_escape(&name.to_uppercase())`,
seats resolved through `st.players`; render `""` instead of the section when
every part below is empty):

1. **Standing pact** (any beat, while pacted):
   `<p class="lc-pact-standing">PACT WITH {NAME} — SINCE ROUND {formed_round}</p>`
2. **Betrayed notice** (any beat, current round only — an entry in
   `pact_breaks` with `betrayed == my_seat && round == st.round`):
   `<p class="lc-pact-broken">{BETRAYER} BROKE YOUR PACT</p>`
3. **Diplomacy-only market**, for an Alive viewer:
   - barred viewer: only
     `<p class="lc-pact-barred">YOU BROKE A PACT — NOBODY DEALS WITH YOU NOW</p>`
   - pacted viewer: nothing beyond the standing line (no market).
   - otherwise, when `alive_count >= PACT_MIN_ALIVE`:
     - one row per pending **incoming** offer:
       `<div class="lc-pact-offer-row"><span>{NAME} OFFERS A PACT</span><button class="lc-btn lc-pact-accept" data-lc-post="pact/accept" data-lc-body="from={seat}">ACCEPT</button><button class="lc-btn lc-pact-decline" data-lc-post="pact/decline" data-lc-body="from={seat}">DECLINE</button></div>`
     - the pending **outgoing** offer, if any:
       `<p class="lc-pact-pending">OFFERED TO {NAME} — WAITING</p>`
     - one button per **proposable** seat — every Alive seat except the
       viewer, publicly-barred seats, and the pending outgoing target
       (its row is the WAITING line above). Secretly-pacted seats are
       included as no-op targets (G11 — the list must not be a pact
       detector):
       `<button class="lc-btn lc-pact-propose" data-lc-post="pact/offer" data-lc-body="target={seat}">PROPOSE TO {NAME}</button>`

No `hx-post`, no `onclick`, no hex — `data-lc-post`/`data-lc-body` are
data-contract attributes, the same adjudication as Plan E's `data-lc-post`.
None of the buttons is `lc-btn-drink` (Global Constraints).

- [ ] **Step 2: the break strip (G5), big screen only**

In `lc_render.rs`'s `lc_screen_panel`, inside the stage (a sibling of the
centre layers; `.lc-stage` is already `position: relative`): for each
`view.pact_breaks` entry with `round == view.round`, in vec order:

```html
<div class="lc-pact-break">{BETRAYER} BROKE THEIR PACT WITH {BETRAYED}</div>
```

Names uppercased + escaped from `view.seats`. Nothing renders for other
rounds — history stays in the state for Plan J's recap. The mini table stays
untouched (the phone's public trace is the big screen and the table's own
noise; the betrayed player additionally gets the private Step-1 notice).
Comment this choice in the builder.

- [ ] **Step 3: `lc_loop.js`** — in the delegated `[data-lc-post]` click
handler, extend the body construction (currently the `data-vessel` branch)
with:

```js
else if (el.dataset.lcBody !== undefined) body = el.dataset.lcBody;
```

Values are server-rendered `key=int` pairs — nothing to encode. Until Task 4
lands the routes, a tapped button POSTs to a 404 and `note()` shows the
fallback text; that one-commit window is acceptable and noted in the ledger.

- [ ] **Step 4: CSS.** In `lastcall.css`, a new section after Plan E's
action-bar rules (tokens only; mint is Diplomacy's hue and therefore the
pact accent; rose marks betrayal):

```css
/* Plan G — pacts. The section renders only in the private hand fragment;
   the break strip is the one public trace a pact leaves before its win
   (decision G10). Pact buttons are never lc-btn-drink — a pact costs no
   drink (G3, E7). */
.lc-pacts { padding: 0 14px 10px; }
.lc-pact-offer-row { display: flex; align-items: center; gap: 8px;
                     padding: 6px 0; font-size: 13px; }
.lc-pact-offer-row span { flex: 1; }
.lc-pact-standing, .lc-pact-pending, .lc-pact-barred, .lc-pact-broken {
  font-family: var(--font-ui); font-weight: 700; font-size: 11px;
  letter-spacing: .12em; padding: 6px 0; }
.lc-pact-standing { color: var(--lc-mint); }
.lc-pact-pending, .lc-pact-barred { color: var(--lc-faint); }
.lc-pact-broken { color: var(--lc-rose); }
.lc-btn.lc-pact-propose { display: block; width: 100%; margin: 4px 0; }
/* the big screen's betrayal line — F.2's floor is 18px */
.lc-pact-break { position: absolute; top: 24px; left: 0; right: 0;
                 text-align: center; font-family: var(--font-ui);
                 font-weight: 700; font-size: 18px; letter-spacing: .14em;
                 color: var(--lc-rose); pointer-events: none; }
```

- [ ] **Step 5: tests.** Unit, in `lc_routes.rs`'s tests module (states built
with the `at_diplomacy` shape inline):

```rust
#[test]
fn test_pacts_section_states() {
    // Fresh 4-player Diplomacy state, viewer alice:
    //   3 PROPOSE TO buttons (BOB, CARA, DAVE), each with
    //   data-lc-post="pact/offer" and data-lc-body="target={1,2,3}";
    //   no ACCEPT, no "PACT WITH", no WAITING.
    // After offer_pact(1, 1):
    //   alice: "OFFERED TO BOB — WAITING", PROPOSE TO CARA and DAVE only;
    //   bob: "ALICE OFFERS A PACT" + ACCEPT/DECLINE with data-lc-body="from=0";
    //   cara: neither string — her section is byte-identical to before the
    //   offer (offers are invisible to third parties).
    // After accept_pact(2, 0):
    //   alice: "PACT WITH BOB — SINCE ROUND 1", no propose buttons;
    //   cara: STILL proposes to ALICE, BOB and DAVE — secretly-pacted seats
    //   stay listed (G11: no pact detector).
    // pact_barred = [3] (public knowledge):
    //   dave's section is exactly the barred line; cara's propose list drops
    //   DAVE.
    // beat = Lock: pacted alice keeps the standing line; unpacted cara gets "".
    // dave eliminated (3 alive < PACT_MIN_ALIVE): unpacted cara gets "".
}

#[test]
fn test_pacts_section_betrayed_notice() {
    // pact_breaks = [{betrayer:0, betrayed:1, round:1}], round 1, beat Lock:
    //   bob's section contains "ALICE BROKE YOUR PACT";
    //   round bumped to 2 -> the notice is gone;
    //   alice at Lock (barred, but the barred line is Diplomacy-only) -> "".
}
```

In `lc_render.rs`'s tests:

```rust
#[test]
fn test_the_break_strip_names_the_knife_for_one_round() {
    // ring_fixture-based view, pact_breaks = [{0, 1, round: 2}]:
    //   view.round == 2 -> exactly one .lc-pact-break, containing both
    //   uppercased seat names and "BROKE THEIR PACT WITH"; no_hex.
    //   view.round == 3 -> zero "lc-pact-break" occurrences.
    //   pact_breaks empty -> zero occurrences.
}
```

In `http.rs` — the fragment half of the mandatory property, driven by
engine-seeded state (`set_game_state` pattern; routes arrive in Task 4). Four
logged-in sessions; the state is built with real player ids, vessels, `beat =
Beat::Diplomacy`, then `offer_pact` + `accept_pact` called on it before
persisting:

```rust
#[tokio::test]
async fn test_the_hand_fragment_shows_only_the_viewers_own_pact() {
    // GET /room/{code}/lastcall/hand as alice: contains "PACT WITH BOB".
    // GET as bob: contains "PACT WITH ALICE".
    // GET as cara: contains NO "PACT WITH" and NO "lc-pact-standing" —
    //   and via without_seq() (Task 4's helper, defined there; if executing
    //   this task first, define it here) her fragment is byte-identical to
    //   the same rig persisted WITHOUT the pact. The route takes no player
    //   identifier — that part is Plan A2's standing property, untouched.
}
```

- [ ] **Step 6: Commit**

```bash
git add drinkinggame/src/lc_routes.rs drinkinggame/src/lc_render.rs drinkinggame/assets/lc_loop.js drinkinggame/assets/lastcall.css drinkinggame/tests/http.rs
git commit -m "feat(lastcall): the pact surfaces — private market in the hand pane, public break strip on the felt"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

### Task 4: The pact routes — and the mandatory secrecy proof

**Class:** C (auth-gated mutations under the per-room lock, and a
broadcast-policy invariant — tick-only, nothing pact-shaped on the shared
stream — that spans surfaces; the reviewer checks "who is subscribed and what
are they looking at" plus the guard chain, per plan-economics §3)

**Why this class:** the three handlers hold the room guard across
load→transition→persist→publish, and the secrecy property being pinned is
exactly the cross-surface kind tests can only sample.

**Files:**
- Modify: `drinkinggame/src/lc_routes.rs` (three handlers + two forms +
  `map_lc` arms)
- Modify: `drinkinggame/src/routes.rs` (route registration beside Plan E's
  `/lastcall/lock` line)
- Test: `drinkinggame/tests/http.rs`

**Interfaces:**
- Consumes: Task 1's transitions; Plan E's handler skeleton
  (`lc_arm_handler`), `lc_lock`, `load_lc`, `persist_and_tick_lc`, `map_lc`,
  `read_sse_until`, the four-session rig from Task 3.
- Produces (exact):

```rust
#[derive(Deserialize)]
pub struct PactOfferForm { pub target: usize }
#[derive(Deserialize)]
pub struct PactFromForm { pub from: usize }

pub async fn lc_pact_offer_handler(State, PlayerSession, Path<String>, Form<PactOfferForm>) -> Response;
pub async fn lc_pact_accept_handler(State, PlayerSession, Path<String>, Form<PactFromForm>) -> Response;
pub async fn lc_pact_decline_handler(State, PlayerSession, Path<String>, Form<PactFromForm>) -> Response;
```

| Method | Path | Body | Publishes |
| --- | --- | --- | --- |
| POST | `/room/{code}/lastcall/pact/offer` | `target` (seat) | tick only |
| POST | `/room/{code}/lastcall/pact/accept` | `from` (seat) | tick only |
| POST | `/room/{code}/lastcall/pact/decline` | `from` (seat) | tick only |

- [ ] **Step 1: handlers.** All three are Plan E's Task 1 skeleton verbatim
(`lc_lock` → guard → `load_lc` → transition → `map_lc` on error →
`persist_and_tick_lc` → `204 NO_CONTENT`) around
`ctx.st.offer_pact(player.id, form.target)` /
`accept_pact(player.id, form.from)` / `decline_pact(player.id, form.from)`.
Tick-only is E5's rule applied a second time, and the reasoning belongs in a
comment on the offer handler: nothing any public surface renders changes
(G10), but both parties' phones need the private re-fetch signal, and the
spectator screen ignores `lctick` by having no listener. Register the three
routes in `routes.rs` beside the existing `lastcall` lines.

- [ ] **Step 2: `map_lc` arms.** Above the catch-all:

```rust
LcError::PactBlocked => {
    (StatusCode::UNPROCESSABLE_ENTITY, "No pact to be had.").into_response()
}
LcError::NoOffer => {
    (StatusCode::UNPROCESSABLE_ENTITY, "That offer is gone.").into_response()
}
```

(Both bodies surface through `lc_loop.js`'s `note()`, like "Can't afford …".)

- [ ] **Step 3: http tests.** The four-session Diplomacy rig from Task 3,
plus this helper beside `read_sse_until` (used by the mandatory test; if Task
3's executor already defined it there, reuse):

```rust
/// Strips every data-seq="N" attribute — the only legitimate byte difference
/// between two renders of the same viewer's fragment across a seq bump.
fn without_seq(body: &str) -> String {
    let mut out = String::with_capacity(body.len());
    let mut rest = body;
    while let Some(i) = rest.find("data-seq=\"") {
        out.push_str(&rest[..i]);
        let after = &rest[i + 10..];
        let close = after.find('"').expect("unterminated data-seq");
        rest = &after[close + 1..];
    }
    out.push_str(rest);
    out
}
```

```rust
/// THE MANDATORY TEST (brief; spec §3.4.1's shape): a pact between A and B
/// is absent from public_view()'s output on the wire, and C's private
/// fragment is unchanged by its existence.
#[tokio::test]
async fn test_a_pact_between_a_and_b_is_invisible_to_c_and_the_wire() {
    // Rig at Diplomacy. GET hand as cara -> cara_before.
    // Open SSE, drain the snapshot with read_sse_until(.., "event: lcpublic").
    // POST pact/offer target=1 as alice -> 204.
    //   read_sse_until(.., "event: lctick"): the newly-read segment carries
    //   NO "event: lcpublic", NO "event: game", NO "event: room" (tick-only).
    // POST pact/accept from=0 as bob -> 204; same tick-only assertion.
    // GET hand as alice: contains "PACT WITH BOB — SINCE ROUND 1".
    // GET hand as bob: contains "PACT WITH ALICE".
    // GET hand as cara -> cara_after:
    //   assert_eq!(without_seq(&cara_before), without_seq(&cara_after));
    //   — her world is byte-identical; the pact does not exist for her.
    // POST /lastcall/handicap (any full-publish route) and
    //   read_sse_until(.., "event: lcpublic"): the frame contains NO
    //   "PACT WITH", NO "lc-pact-standing", NO "lc-pacts" — the broadcast
    //   surface never carries pact state even while a pact exists.
}

#[tokio::test]
async fn test_pact_routes_are_guarded_and_answer_in_words() {
    // carol (logged in, NOT a member): POST pact/offer -> 403 (load_lc).
    // A three_man room: POST pact/offer -> 409 (WrongGameKind).
    // Rig at Beat::Lock: POST pact/offer -> 409 (WrongBeat -> OutOfTurn).
    // Rig at Diplomacy: POST pact/accept from=3 with no offer -> 422, body
    //   exactly "That offer is gone.".
    // alice+bob pacted (via routes), cara: POST pact/offer target=0 -> 204
    //   (G11 — a no-op offer, never an error); then alice POST pact/offer
    //   target=2 -> 422 "No pact to be had." (pacted offeror).
}

#[tokio::test]
async fn test_decline_clears_the_offer_for_both_phones() {
    // POST pact/offer target=1 as alice; GET hand as bob: "ALICE OFFERS A
    // PACT" with data-lc-body="from=0". POST pact/decline from=0 as bob ->
    // 204 (tick-only, same frame assertion as above). GET hand as bob: the
    // offer row is gone; GET hand as alice: no "WAITING", and "PROPOSE TO
    // BOB" is back.
}

#[tokio::test]
async fn test_the_break_is_the_only_public_pact_trace() {
    // Seed (set_game_state): round 1, pact_breaks=[{betrayer:0, betrayed:1,
    // round:1}] AND an intact pact {a:2, b:3} in the same state. POST
    // /lastcall/handicap; read_sse_until(.., "event: lcpublic"): the frame
    // contains "lc-pact-break" and "BROKE THEIR PACT WITH" (the betrayal is
    // public, G5) and NO "lc-pact-standing", NO "SINCE ROUND" (the intact
    // pact is not, G10).
}
```

Name in the task report that `test_rof_sse_snapshot_has_no_lcpublic`,
`test_tm_sse_snapshot_includes_tm_panels` and
`test_sse_snapshot_has_all_stateful_kinds` are still green (route table
change).

- [ ] **Step 4: Commit**

```bash
git add drinkinggame/src/lc_routes.rs drinkinggame/src/routes.rs drinkinggame/tests/http.rs
git commit -m "feat(lastcall): pact routes under the room guard — tick-only, and the secrecy proof on the wire"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

## Browser checkpoint — after Task 4, before the final review

A human, a real focused browser, `cargo run -p drinkinggame`, **four** browser
profiles in one room (PACT_MIN_ALIVE is 4), a Last Call game driven to
Diplomacy via START ROUND 1, plus the big screen in a fifth window:

1. Phone A proposes to B: A's row flips to OFFERED — WAITING; B sees the
   ACCEPT/DECLINE row appear on the SSE tick without a reload; C and D see
   nothing change; the big screen shows nothing.
2. B accepts: both A and B show PACT WITH … — SINCE ROUND 1; C's and D's
   panes still list A and B as proposable (G11); the spectator screen is
   still silent about it.
3. C proposes to the pacted A: 204, C sits at WAITING forever; the beat's end
   clears it without a trace.
4. Decline round-trip: D proposes to C, C declines, D's row reverts to
   PROPOSE TO C.
5. Betray: at Lock, A arms a single-target attack at B, locks; watch the
   resolve — the big screen shows "{A} BROKE THEIR PACT WITH {B}" for that
   round only, B's phone shows "{A} BROKE YOUR PACT", and next Diplomacy A's
   pane shows the barred line while everyone else's propose list drops A.
6. The pact win: re-pact two players, eliminate the other two — the felt
   shows "{A} & {B} — THE PACT HOLDS", the banner says GAME OVER, END GAME
   still hands off to the idle room panel.
7. Ring of Fire and 3 Man still behave exactly as before (one quick room of
   each — route-table change).

## Before the plan is done

- Every task's `./scripts/verify.sh` output quoted; tests grow from the
  post-Plan-E baseline recorded in the ledger; clippy still **17 distinct**
  warnings, all `drawingportfolio`; `drinkinggame` stays clean.
- Task 4 (Class C) had its per-task reviewer on a capable model; Tasks 1–3
  (Class B) are covered by **one whole-plan review** of the branch diff at
  the end, on the most capable model (plan-economics §4). The reviewer brief
  must name: the `public_view()` pact discipline (three private fields, one
  deliberate public one), the tick-only publish policy against "who is
  subscribed", the G11 no-pact-detector property (propose lists, no-op
  offers, and the `without_seq` byte-identity test), the guard chains'
  order, and that `persist_and_tick_lc` stays await-free under the guard.
- The browser checkpoint run by a human (above).
- No `cargo sqlx prepare` (no migration; runtime-checked queries).
- Interfaces line up: Task 1's transitions are what Task 4's handlers call
  and Task 3's section renders around; Task 2's `pact_breaks` projection is
  what Task 3's strip consumes; Task 2's `LcOutcome::Pact` arm is the
  compile-forced extension of Plan E's game-over centre; `data-lc-body` is
  written by Task 3's builder and read by Task 3's JS line.
- Every brief requirement maps: full pact design proposed → G1–G14; engine
  extension with secrecy-safe fields → Tasks 1–2; routes under the guard,
  tick-only, no player identifier → Task 4; per-viewer UI + CSS + JS in
  `lc_loop.js` → Task 3; the MANDATORY §3.4.1-shaped test → Task 4 (wire +
  third-player fragment) with Task 1's unit twin.
- STATUS updated: content system 1 shipped; G1's divergence from DDv2 §10.3
  recorded as awaiting user review; Plan J's inherited items noted (end-screen
  pact recap, G14) and the spec-§9 pacts gap marked closed by G2.

## Self-review (performed while writing)

- **Design grounding:** every bundle sentence on pacts was read and either
  adopted (DDv1's win condition, secrecy, safety-valve spirit, "two tabs and
  a shared win check" cheapness) or divergence-flagged with the quote (G1
  against DDv2 §10.3's silent pairing; G6 against DDv1's one-per-game valve;
  G13/G14 name what is dropped or deferred and why). The spec-§9 recorded
  gap (no win condition) is closed by G2 and flagged.
- **Secrecy audit:** `pacts`/`pact_offers`/`pact_barred` unprojected (Task 1
  test, narrowed in Task 2 as planned); `pact_breaks` public by explicit
  decision (G5); offers/accepts tick-only (E5's rule, Task 4 frame tests);
  the private section rides the no-identifier hand route; the G11 analysis
  closed the two leak channels found while writing (market-list exclusion
  and offer-refusal probing) — both are now decisions with tests.
- **Placeholder scan:** no TBD/TODO/"handle appropriately"; every route
  path, form field, error body, copy string, guard order and test expected
  value is spelled out; prose only where a named pattern exists (Plan E's
  handler skeleton, `targets_section_html`, the `add_player` idempotence
  precedent).
- **Type consistency:** `Pact`/`PactOffer`/`PactBreak` defined in Task 1,
  consumed by Tasks 2–4; `pact_partner` defined Task 1, used Task 2 (betrayal)
  and Task 3 (section); `PactOfferForm`/`PactFromForm` match the seat-typed
  bodies Task 3's `data-lc-body` values encode; `LcOutcome::Pact(usize,
  usize)` serde name pinned; consumed C/D/E/F names copied from their
  Produces blocks with the unexecuted-plans caveat stated.
- **Sizing:** 4 tasks (B, B, B, C), one browser checkpoint, ends deployable —
  a room can play Last Call with pacts end to end.
