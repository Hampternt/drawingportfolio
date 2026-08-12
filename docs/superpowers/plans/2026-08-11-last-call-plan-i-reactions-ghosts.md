# Last Call — Plan I: reactions and ghosts (slice 4c)

> **For agentic workers:** REQUIRED SUB-SKILLS: `plan-economics` (this repo's
> task classes, review policy and plan sizing) then
> superpowers:subagent-driven-development to execute task-by-task.

**Goal:** Build the two out-of-turn systems — the reaction response window
(DDv2 §7.3–7.4) and ghosts (§9.2) — so Plan F's five inert reaction cards do
real work and an eliminated player keeps a seat at the table.

**Architecture:** The response window is not a new phase: DDv2's reactions and
ghost votes both act on "plays in flight", and plays are in flight during
exactly one beat — `Beat::Reveal`, the 20s timed beat Plan D flips plays public
into and Plan E's chain resolves out of. Both systems are pure engine
transitions (`play_reaction`, `haunt`) on new container-`#[serde(default)]`
fields (`reactions`, `haunts`) that Plan D's seq discipline and Plan E's route
shape carry unchanged. A played reaction is public the instant it exists —
it never enters `plays` (which is what makes TBD-7 structural) — and an
unplayed one is ordinary hand state, private by the §3.3 fetch pattern.
Reaction rules live in the catalog (`CardDef.rfx`, the F1 move again), applied
by `resolve()` as play-queue modifiers, not as `EffectOp`s — they subject
*plays*, not players.

**Slice:** When this plan is done, the Reveal beat is live: a player holding a
reaction sees a response section in their hand pane and can cancel, blunt or
reflect a revealed play (charged in pulls, on the spot, LIFO per play,
unreactable-to); an eliminated player's phone offers one haunt per round that
rides +1 onto an attack in flight; both acts land on the big screen as chips on
the centre plays; and each public response holds the window open at least 10
more seconds. Events/tabs (Plan H) and pacts (Plan G) are untouched.

**Execution order — binding:** This plan runs **after Plan E** (it consumes
E's routes file shape, `map_lc`, `persist_and_broadcast_lc`, the beat clock's
`beat_deadline_ms`, the action bar, and `lc_loop.js`) and therefore after
Plans C, D and F, which E builds on. It consumes **nothing** from Plan G
(pacts) or Plan H (events + tabs): no shared interface, no shared beat —
events land at beat 2, pacts at beat 3, this plan lives at beat 5 — and all
three add only container-level-default fields to `LastCallState`, so they
commute in any order. If G/H have not executed, nothing here waits for them.
Plan J's **public-only game log** is honoured by construction: everything this
plan stores outside a hand (`reactions`, `haunts`) is public from the moment
it exists, so all of it is log-safe; the only secret — an *unplayed* reaction
in hand — never leaves the private fetch (Task 2's §3.4.1-shape test is the
pin, and no log may ever say "N could have reacted").

**Ledger:** `.superpowers/sdd/2026-08-11-last-call-plan-i-reactions-ghosts/progress.md`
(gitignored).

---

## Proposed design decisions — awaiting user review

DDv2 §7.3 names the response window but never opens it; §9.2 gives ghosts a
rule but no UI or timing. Everything below is proposed here, implemented as
written, and reviewed by the user after execution. Every number is a named
constant.

- **I1 — The Reveal beat IS the response window.** Reactions and ghost votes
  are legal during `Beat::Reveal` and nowhere else. Rationale: §7.3's window
  must sit between the flip and resolution, and that interval already exists
  as a timed beat — Plan D flips plays public at Lock→Reveal, Plan E's ticker
  resolves at Reveal's deadline, and E3 explicitly reserved Reveal's 20s as
  "the look-at-the-flip pause (no reaction system exists to fill it)". This
  plan fills it. **No change to `lc_advance_chain`**: Reveal is a timed beat,
  so the chain already stops there; the Reveal→Resolve→`resolve()` collapse at
  deadline is untouched.
- **I2 — The window opens every round, for the full duration, regardless of
  who holds what.** A conditional window ("only opens when a revealed play is
  reactable and someone holds a reaction") would leak *someone holds a
  reaction* through its very existence — §6.1's remove-the-input principle,
  applied to time. Likewise there is no "all passed" early exit: knowing
  everyone passed requires knowing who *could* have acted. Cost: 20 dead
  seconds in rounds where nobody responds, which doubles as the
  look-at-the-flip pause E3 already budgeted. **Flagged** — this is the
  privacy/pacing trade the user should weigh.
- **I3 — A public response extends the window: `REACT_GRACE_SECS` (10).**
  Each successful react or haunt raises `beat_deadline_ms` to at least
  `now + 10s` (never lowers it). Triggered only by public events, so it leaks
  nothing; without it a reaction at 0:01 resolves before the table has seen
  it. Route-side write under the room guard, exactly like `arm_beat_clock`
  (E2: the deadline is route-owned engine *data*). **Flagged** — a table that
  keeps reacting keeps the beat open, by design.
- **I4 — Reaction rules are a catalog column, `CardDef.rfx: Option<ReactionFx>`**
  with `ReactionFx { Cancel, Reduce(i32), Reflect }` — the F1 move (rules from
  the binary, never the blob). Not `EffectOp`: effects subject players and
  persist on the room; reactions subject *plays* and die with the round. A new
  `EffectOp` would be the wrong shape and would leak into F7's keyword
  predicates.
- **I5 — Reaction scope reuses the card's `targets` field:** `"self"` = may
  answer only a play whose subjects include you; `"one"` = may answer **any**
  play, including on someone else's behalf (DDv1 §11's open question, answered
  yes — it is what the trickster and control decks sell). `Reflect`
  additionally requires a seat-targeted play (`play.target.is_some()`) — an
  aoe has no single target to swap; refused at play time (`BadTarget`), the
  card stays in hand.
- **I6 — The reaction fx table** (Plan F shipped these cards inert, ×2 copies;
  this plan arms them — copies stay ×2 per F5, so arming is a pure buff to
  cards people already hold):

  | id | title | cost | scope | fx | rationale |
  | --- | --- | --- | --- | --- | --- |
  | beer-08 | Coaster | 1 | self | Reduce 3 | attrition blunts cheaply; 3 for 1 pull beats par (2/pull) because it only ever protects you |
  | cider-08 | Not So Fast, Friend | 2 | any | Cancel | the trickster counterspell — 2 pulls to void up to 8 damage is the deck's whole tempo identity |
  | wine-08 | Send It Back | 2 | any, seat-targeted plays only | Reflect | control's signature: the table's scariest card now has a scariest answer |
  | liquor-08 | Spit It Out | 2 | self | Cancel | burst defends in bursts; self-only at the same cost as cider's any — the 4-pull vessel already prices scope |
  | soft-04 | The Long Sober Look Across The Table | 1 | self | Reduce 4 | support's protection premium (F3: 2.5–3.0 per pull) |

  Cider-08 strictly outclasses liquor-08 on scope at equal cost — deliberate
  deck identity, noted for playtest.
- **I7 — Reactions are charged when played,** `pull_cost(cost, handicap)` from
  the fullest vessel of the card's deck (D3's greedy rule, one card). They
  never touch the 7.1 ordering total: order keys were assigned at the reveal
  and D4's rationale extends — the totals the flip made public do not move
  afterwards.
- **I8 — TBD-7 is structural, and §12's double-cancel is LIFO fizzle.** A
  reaction answers a play by `order_key`; played reactions live in
  `LastCallState.reactions`, never in `plays`, and carry no order_key — so
  "react to a reaction" is unrepresentable rather than guarded (§6.1 again).
  At resolution, reactions answering one play apply last-in-first-out (§7.3);
  a Cancel on an already-cancelled play fizzles — spent, discarded, no effect
  (§12). Reactions, once played, stand even if the reactor is later eliminated
  in the same resolution — §12's removal rule names *plays*, and the reaction
  was paid for while alive.
- **I9 — §3.4.1 discipline for reactions:** an unplayed reaction is hand state
  and never reaches any projected field; `play_reaction` reveals and stores in
  the same mutation — the card becomes public at the exact moment it enters
  engine state. Nothing unrevealable ever sits where `public_view()` can see
  it. (The public `hand_len` drops by one — honest: the play was public.)
- **I10 — Ghosts get DDv2 §9.2 verbatim and nothing more:** one action,
  `haunt(order_key)` — once per round, `Status::Eliminated` players only,
  `HAUNT_BONUS` (+1) damage onto a damage play in flight. No emotes, no
  nudges, no card play: §9.2 is the only ghost text in the bundle and it
  specifies exactly this. The balance constraint honoured: a ghost can only
  amplify a living player's attack by 1 per round — it cannot create, target,
  drink, or be targeted (already enforced: `set_target` rejects Eliminated
  seats, dead-target plays fizzle, elimination discards the hand).
- **I11 — Resolution math, per play at its slot:** cancelled → the whole play
  resolves as nothing (card discarded, pulls stay spent — 7.5 parity; haunt
  votes on it are wasted). Otherwise Reflect swaps the subject set to the
  source seat, then per-subject damage =
  `max(0, magnitude + HAUNT_BONUS × votes − that subject's Reduce total)`,
  then shields, then HP — reductions protect the reactor's own seat (both
  Reduce cards are self-scope, so this is total). Reduce answering a
  non-damage play is wasted, like every mistimed reaction: play-time guards
  check structure, never wisdom.
- **I12 — Ghost and reaction UI live inside existing surfaces.** The response
  section joins the hand pane's private fetch (beside E8's target picker); the
  haunt buttons take over Plan E's `!alive` action-bar row during Reveal; the
  felt-centre plays grow public chips (reactor + card, ghost +1). No new tab,
  no new page, no new SSE event, and — deliberately — **no new flights**:
  reaction/haunt presentation is static chips this plan, the same deferral E15
  made for discard flights.

---

## Global Constraints

Every task's requirements implicitly include this section.

**Spec bindings carried from slices 1–3 — all still in force:**

- No private route or mutating route takes a player identifier; the actor is
  the session cookie, targets are engine-validated values (§6.1). The two new
  routes name a card and a play, never a player.
- Spec §3.4.1: nothing enters `plays` before it is revealable. This plan adds
  **no** writer to `plays` at all — reactions live in their own field (I8/I9)
  and Task 2 owns the secrecy test.
- Publish order `game → room → lcpublic → lctick` inside
  `persist_and_broadcast_lc` is unchanged; `broadcast_lc` stays await-free
  under the room guard (`1e742d4`); **no new SSE event**; SSE tests filter by
  content via `read_sse_until`, never index positionally.
- Public renderers take `&PublicView`; renderers emit deck class names, never
  hex — new builders join the `no_hex` sweep. `ActionBarView` stays
  private-side, per-viewer, never broadcast.
- seq discipline: every successful mutating transition bumps `seq` exactly
  once; failures bump nothing.
- Guard order is part of the interface — documented per transition below,
  pinned by tests, trusted by `map_lc`.
- D9 stands permanently: reactions cannot be **armed** (`NotPlayable` at
  `arm`) — §7.3 makes them "the only cards playable outside beat 4", i.e.
  never beat-4 cards. Task 2 updates D9's comment (the rationale is no longer
  "the window is a later slice"; it is "reactions live in the window").
- Ring of Fire and 3 Man untouched; the named pins
  (`test_rof_sse_snapshot_has_no_lcpublic`,
  `test_tm_sse_snapshot_includes_tm_panels`,
  `test_sse_snapshot_has_all_stateful_kinds`) stay green after every task.
- One `@media (prefers-reduced-motion: reduce)` block in `lastcall.css`; this
  plan adds no animation, so it must not touch that block at all.
- Keep `drinkinggame` clippy-clean; the 17 pre-existing distinct warnings are
  all in `drawingportfolio` and the count must not grow.

**Consumed from earlier plans (exact — build against their Produces blocks;
Plans C–F and E are written but may be unexecuted when this plan is written —
if any signature drifted during their execution, reconcile toward the tree
and note it in the task report):**

```rust
// Plan D:
pub fn arm(&mut self, player_id: i64, card_id: &str) -> Result<(), LcError>;
pub fn advance_beat(&mut self) -> Result<(), LcError>;
pub fn resolve(&mut self) -> Result<(), LcError>;
pub enum LcError { NotSeated, BadHandicap, WrongBeat, NotAlive, AlreadyLocked,
    UnknownCard, NotPlayable, CantAfford(String), NeedsTarget(String),
    BadTarget, BadDraw, MustResolve }
pub fn pull_cost(cost: u8, handicap_pct: u16) -> u8;   // exists since Plan A
pub struct Play { pub card: Card, pub source_seat: usize,
    pub target: Option<usize>, pub paid_from: Deck, pub order_key: u32 }
// LastCallState: container-level #[serde(default)]; locked_plays never
// projected; Beat::Reveal is where the Lock→Reveal edge flips plays public.

// Plan F:
pub struct CardDef { /* id, deck, kind, cost, targets, title, text,
    keywords, copies, fx: Option<FxDef> */ }
pub fn card_fx(id: &str) -> Option<FxDef>;  // None for reactions & unknown ids
pub fn card_by_id(id: &str) -> Option<Card>;
// The five reactions: beer-08 c1 self / cider-08 c2 one / wine-08 c2 one /
// liquor-08 c2 self / soft-04 c1 self — fx: None, copies 2, kw ["reaction"].

// Plan E:
pub(crate) fn map_lc(e: LcError) -> axum::response::Response;
pub(crate) async fn persist_and_broadcast_lc(state: &GameState, ctx: &LcCtx);
pub(crate) fn now_ms() -> i64;
// LastCallState.beat_deadline_ms: Option<i64> — route-owned data (E2).
// lc_routes: lc_lock / load_lc / LcCtx; the Task 1 handler skeleton.
// lc_render::ActionBarView { beat, round, seated, alive, locked, drawing,
//     vessels, charged, vessels_registered, outcome } + lc_action_bar(&ab);
// lc_routes: action_bar_view(st, player_id), targets_section_html(st, seat),
//     hand_pane_html appends extras after the #lc-hand root fragment.
// lc_loop.js: delegated [data-lc-post] click posting to
//     /room/{code}/lastcall/{action}; note() surfacing 422 bodies; the E7
//     eliminated row "YOU'RE OUT — HAUNT THE TABLE".
// REVEAL_SECS = 20; lc_advance_chain stops at timed beats (Reveal included).
```

**Baseline:** whatever Plan E's ledger records at its close — the pre-C/D
figure was 371 tests; the invariants are *`./scripts/verify.sh` green* and
*17 distinct clippy warnings, all `drawingportfolio`, `drinkinggame` clean*,
not a fixed count. Read the number from
`.superpowers/sdd/2026-08-11-last-call-plan-e-loop-wiring/progress.md` and
record it in this plan's ledger before starting. No migration, no
`cargo sqlx prepare` (`drinkinggame` is runtime-checked).

**New constants** (each at the site that owns it):

```rust
// last_call.rs — DDv2 9.2's "+1 damage": the whole ghost power, playtest-movable.
pub const HAUNT_BONUS: i32 = 1;
// lc_routes.rs — decision I3: a public response holds the window open this long.
pub(crate) const REACT_GRACE_SECS: u16 = 10;
```

**Verification for every task:** `./scripts/verify.sh` — all green, output
quoted in the report. Never a bare `cargo test`.

**Browser checkpoint: one**, after Task 5, before the final review. Not per
task.

---

### Task 1: Arm the catalog — `ReactionFx`, the `rfx` column, real rules text

**Class:** B (logic, tests specified below)

**Why this class:** static data plus pure lookups; every invariant is a
predicate over the catalog with expected values written here.

**Files:**
- Modify: `drinkinggame/src/lc_cards.rs`
- Test: `drinkinggame/src/lc_cards.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: Plan F's `CardDef`, `CATALOG`, the five reaction rows.
- Produces (exact — Tasks 2 and 5 build against these):

```rust
/// A reaction's rules, resolved by id at play time — catalog-side like FxDef
/// (F1): never stored in the blob, so a retune reaches in-flight games.
/// Applied by resolve() as play-queue modifiers (decision I4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReactionFx {
    /// The answered play resolves as nothing (7.5 parity: pulls stay spent).
    Cancel,
    /// The answered play deals this much less damage to the reactor.
    Reduce(i32),
    /// The answered play resolves against its own source instead.
    Reflect,
}

// CardDef gains:
pub rfx: Option<ReactionFx>,   // Some ⇔ kind == Reaction (test-pinned)

pub fn card_rfx(id: &str) -> Option<ReactionFx>; // None for unknown ids too
```

- [ ] **Step 1: The column and the five rows**

Add `rfx: Option<ReactionFx>` to `CardDef` and `rfx: None,` to all 35
non-reaction rows (mechanical). The five reaction rows get the I6 table as
data plus new `text` — the F "inert until the response window ships" strings
die, replaced verbatim (the no-flavor-only rule: text states the effect):

```rust
// beer-08  — rfx: Some(ReactionFx::Reduce(3)),
text: "Reaction: a revealed play deals 3 less damage to you. Slide it over your glass.",
// cider-08 — rfx: Some(ReactionFx::Cancel),
text: "Reaction: cancel any revealed play, whoever it was aimed at. Keep it where they can see it.",
// wine-08  — rfx: Some(ReactionFx::Reflect),
text: "Reaction: a revealed play aimed at one player resolves against its owner instead. Summon the sommelier.",
// liquor-08 — rfx: Some(ReactionFx::Cancel),
text: "Reaction: cancel a revealed play aimed at you. Undignified but effective.",
// soft-04  — rfx: Some(ReactionFx::Reduce(4)),
text: "Reaction: a revealed play deals 4 less damage to you. You know what you did.",
```

Costs, kinds, `targets` values, titles, keywords and `copies` (×2) are
untouched — F5's activation toggle stays flipped on, and arming live cards is
the point. `card_rfx` mirrors `card_fx`:
`CATALOG.iter().find(|d| d.id == id).and_then(|d| d.rfx)`.

Update the module doc's F5 paragraph: reactions are no longer inert; the
response window is Beat::Reveal (this plan), and `rfx` is their rules column.

- [ ] **Step 2: Tests**

Extend Plan F's `test_fx_matches_kind` — the `(CardKind::Reaction, None) => {}`
arm stays (fx is still `None` for reactions), and the test gains the rfx
bidirectional pin:

```rust
for def in CATALOG.iter() {
    assert_eq!(def.rfx.is_some(), def.kind == CardKind::Reaction, "{}", def.id);
}
```

New test, the I6 table as expected values:

```rust
#[test]
fn test_reaction_fx_table() { // decision I6 — arming F5's inert cards
    assert_eq!(card_rfx("beer-08"), Some(ReactionFx::Reduce(3)));
    assert_eq!(card_rfx("cider-08"), Some(ReactionFx::Cancel));
    assert_eq!(card_rfx("wine-08"), Some(ReactionFx::Reflect));
    assert_eq!(card_rfx("liquor-08"), Some(ReactionFx::Cancel));
    assert_eq!(card_rfx("soft-04"), Some(ReactionFx::Reduce(4)));
    assert_eq!(card_rfx("beer-01"), None);
    assert_eq!(card_rfx("nope"), None);
    // The inert-era text is gone from every reaction:
    for def in CATALOG.iter().filter(|d| d.kind == CardKind::Reaction) {
        assert!(!def.text.contains("inert"), "{}", def.id);
        assert!(def.text.starts_with("Reaction:"), "{}", def.id);
    }
}
```

The §9 coverage floor is untouched: titles did not change (soft-04's 36-char
title still holds the >24 band), and `test_catalog_has_an_overflowing_body`
only requires wine-01's membership, which stands whatever these texts weigh.

- [ ] **Step 3: Commit**

```bash
git add drinkinggame/src/lc_cards.rs
git commit -m "feat(lastcall): arm the reaction cards — ReactionFx column, real rules text"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

### Task 2: Engine — `play_reaction` and the LIFO resolution pass

**Class:** B (logic, tests specified below — including the mandatory window,
TBD-7 and §3.4.1-shape secrecy tests, all encodable as exact-value/JSON-absence
assertions; nothing here locks or broadcasts)

**Files:**
- Modify: `drinkinggame/src/last_call.rs`
- Test: `drinkinggame/src/last_call.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: Task 1's `card_rfx`/`ReactionFx`; Plan D's `Play`, guard-order
  conventions, `pull_cost`; Plan F's `card_fx` (already called by `resolve()`).
- Produces (exact — Tasks 3–5 build against these):

```rust
/// A played reaction: public from the moment it exists (decision I9).
/// `answers` is the order_key of the play it interrupts — plays are the only
/// things with order keys, so a reaction can never be named (TBD-7, I8).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ReactionPlay {
    pub card: Card,
    pub source_seat: usize,
    pub answers: u32,
}
// LastCallState gains (container serde(default) keeps old blobs loading):
//   pub reactions: Vec<ReactionPlay>,
// PublicView gains the same field, projected verbatim (all of it is public).

pub fn play_reaction(
    &mut self,
    player_id: i64,
    card_id: &str,
    answers: u32,
) -> Result<(), LcError>;
```

- [ ] **Step 1: `play_reaction`**

Guard order (documented in the doc comment; the TBD-7 test relies on
`BadTarget` outranking `CantAfford`):
`NotSeated` → `NotAlive` (ghosts hold no cards, 9.2) → `WrongBeat` (beat must
be `Beat::Reveal` — decision I1, the window and nothing else) → `UnknownCard`
(id not in hand) → `NotPlayable` (kind != `CardKind::Reaction` — the mirror of
D9: only reactions play here, reactions play only here) → `BadTarget` (no play
in `plays` with `order_key == answers`) → `BadTarget` (scope, I5: if
`card.targets == "self"`, the answered play's subject set must include the
reactor's seat — subjects per D2 from the play's `target`/`card.targets`,
where "all" includes everyone; if `card_rfx` is `Reflect`, the play's `target`
must be `Some(_)`) → `CantAfford(card_id)` (single-card D3 greedy:
`pull_cost(card.cost, handicap_pct)` from the fullest vessel of `card.deck`,
tie → lowest index).

Success, one mutation: deduct the pulls, move the card from `hand` into
`reactions` as `ReactionPlay { card, source_seat, answers }`, `seq += 1`. The
card is revealed and stored in the same step — nothing unrevealed ever sits in
a projected field (I9). `public_view()` projects `reactions` verbatim.

Update D9's comment at the `arm` guard: reactions are beat-5 cards played via
`play_reaction`; arming them is permanently illegal, not provisionally.

- [ ] **Step 2: The resolution pass inside `resolve()`**

At the top of the play-resolution step, fold `reactions` (decision I11):

- Process reactions **per answered play, in reverse played order** (LIFO,
  §7.3): `Cancel` marks the play cancelled — on an already-cancelled play it
  fizzles (§12), no error; `Reduce(n)` accrues `n` against
  `(answers, reactor_seat)`; `Reflect` marks the play reflected (idempotent —
  a second reflect re-marks the same source).
- Per play at its slot: **cancelled** → no fx at all; the card still discards,
  pulls stay spent (7.5 parity), any haunt votes are wasted (Task 3).
  Otherwise, **reflected** → the subject set becomes `[source_seat]`.
  For `EffectOp::Damage`, per subject `s`:
  `max(0, magnitude − reduce_total(play, s))` — Task 3 adds the vote term —
  then shields, then HP, as Plan F left it. Non-damage ops ignore reductions
  (a Reduce answering a heal was wasted, I11).
- After the play loop: drain every `ReactionPlay.card` into `discards`
  (reactions die with the round, resolved or wasted alike — 8.4 parity).

Reactions from a reactor who was eliminated mid-resolution still apply (I8 —
§12 removes *plays*, and the reaction was paid while alive); this falls out of
pre-computing the fold before the loop, note it in the comment.

- [ ] **Step 3: Tests**

Shared fixture beside Plan D's `at_lock` (Plan F openers hold — reaction cards
are never in an opener, so tests deal them in by hand):

```rust
/// alice(1)/Beer locks beer-02 (Damage 4 → bob) and is revealed: one play,
/// order_key 1, alice charged 2 (Beer 8→6). bob(2)/Cider holds cider-08
/// (Cancel, any); cara(3)/Soft holds soft-04 (Reduce 4, self).
fn at_reveal() -> LastCallState {
    let mut st = at_lock();
    st.players[1].hand.push(crate::lc_cards::card_by_id("cider-08").unwrap());
    st.players[2].hand.push(crate::lc_cards::card_by_id("soft-04").unwrap());
    st.arm(1, "beer-02").unwrap();
    st.set_target(1, "beer-02", Some(1)).unwrap();
    st.lock_in(1).unwrap();
    st.lock_in(2).unwrap();
    st.lock_in(3).unwrap();
    st.advance_beat().unwrap(); // Lock → Reveal
    st
}
```

```rust
#[test]
fn test_play_reaction_charges_and_stores_public() {
    let mut st = at_reveal();
    let seq = st.seq;
    st.play_reaction(2, "cider-08", 1).unwrap();
    assert_eq!(st.players[1].hand.len(), 5);            // 6 → 5
    assert_eq!(st.players[1].vessels[0].pulls_left, 8); // Cider 10 − 2
    assert_eq!(st.reactions.len(), 1);
    assert_eq!(st.reactions[0].answers, 1);
    assert_eq!(st.plays.len(), 1);                      // plays untouched (I8)
    assert_eq!(st.seq, seq + 1);
    assert_eq!(st.public_view().reactions.len(), 1);    // public immediately (I9)
}

/// MANDATORY — window-only legality, the exact error.
#[test]
fn test_a_reaction_outside_the_window_is_rejected() {
    let mut st = at_reveal();
    for beat in [Beat::Draw, Beat::Deal, Beat::Diplomacy, Beat::Lock, Beat::Resolve] {
        st.beat = beat;
        assert_eq!(
            st.play_reaction(2, "cider-08", 1),
            Err(LcError::WrongBeat),
            "beat={beat:?}"
        );
    }
    st.beat = Beat::Reveal;
    st.play_reaction(2, "cider-08", 1).unwrap(); // and the window itself works
}

/// MANDATORY — §3.4.1-shape secrecy: an unplayed reaction never reaches the
/// projection; a played one is public in the same mutation.
#[test]
fn test_an_unplayed_reaction_never_reaches_the_public_view() {
    let mut st = at_reveal(); // bob HOLDS cider-08, nobody has played it
    let json = serde_json::to_string(&st.public_view()).unwrap();
    assert!(!json.contains("cider-08"));
    assert!(!json.contains("Not So Fast"));
    st.play_reaction(2, "cider-08", 1).unwrap();
    let json = serde_json::to_string(&st.public_view()).unwrap();
    assert!(json.contains("cider-08")); // revealed exactly when it exists
}

/// MANDATORY — TBD-7 / DDv2 7.4, pinned structurally: a reaction has no
/// order_key and never enters `plays`, so there is nothing to name.
#[test]
fn test_a_reaction_cannot_be_reacted_to() {
    let mut st = at_reveal();
    st.play_reaction(2, "cider-08", 1).unwrap();
    assert_eq!(st.plays.len(), 1); // the reaction is NOT among the plays
    // cara tries to answer it — the only names that exist are play keys, and
    // the reaction has none. (BadTarget outranks CantAfford in the guard
    // order, so cara's missing Cider vessel never masks the refusal.)
    st.players[2].hand.push(crate::lc_cards::card_by_id("cider-08").unwrap());
    assert_eq!(st.play_reaction(3, "cider-08", 2), Err(LcError::BadTarget));
}

#[test]
fn test_reaction_guards() {
    let mut st = at_reveal();
    assert_eq!(st.play_reaction(999, "cider-08", 1), Err(LcError::NotSeated));
    assert_eq!(st.play_reaction(2, "nope", 1), Err(LcError::UnknownCard));
    // A non-reaction card in hand cannot ride the window:
    assert_eq!(st.play_reaction(2, "cider-01", 1), Err(LcError::NotPlayable));
    // Self-scope: the play targets bob, cara is not among its subjects:
    assert_eq!(st.play_reaction(3, "soft-04", 1), Err(LcError::BadTarget));
    // CantAfford names the card (cara holds a cider-08 but no Cider vessel):
    st.players[2].hand.push(crate::lc_cards::card_by_id("cider-08").unwrap());
    assert_eq!(
        st.play_reaction(3, "cider-08", 1),
        Err(LcError::CantAfford("cider-08".into()))
    );
    // Ghosts hold no cards (9.2):
    st.players[1].status = Status::Eliminated;
    assert_eq!(st.play_reaction(2, "cider-08", 1), Err(LcError::NotAlive));
}

#[test]
fn test_cancel_voids_the_play_but_not_the_pulls() {
    let mut st = at_reveal();
    st.play_reaction(2, "cider-08", 1).unwrap();
    st.advance_beat().unwrap(); // Reveal → Resolve
    st.resolve().unwrap();
    assert_eq!(st.players[1].hp, 15);                   // cancelled
    assert_eq!(st.players[0].vessels[0].pulls_left, 6); // 7.5: no refund
    assert_eq!(st.discards.len(), 2);                   // beer-02 + cider-08
    assert!(st.reactions.is_empty());
    assert_eq!(st.round, 2);                            // rollover unbothered
}

#[test]
fn test_reduce_blunts_for_the_reactor_and_floors_at_zero() {
    // Full absorb: cider-05 (Damage 4) → cara, soft-04 Reduce 4 → 0.
    let mut st = at_lock();
    st.players[2].hand.push(crate::lc_cards::card_by_id("soft-04").unwrap());
    st.arm(2, "cider-05").unwrap();
    st.set_target(2, "cider-05", Some(2)).unwrap();
    st.lock_in(1).unwrap(); st.lock_in(2).unwrap(); st.lock_in(3).unwrap();
    st.advance_beat().unwrap();
    st.play_reaction(3, "soft-04", 1).unwrap();          // Soft 6 − 1 = 5
    st.advance_beat().unwrap();
    st.resolve().unwrap();
    assert_eq!(st.players[2].hp, 15);

    // Partial: cider-05 (4) → alice, beer-08 Reduce 3 → 1 through.
    let mut st = at_lock();
    st.players[0].hand.push(crate::lc_cards::card_by_id("beer-08").unwrap());
    st.arm(2, "cider-05").unwrap();
    st.set_target(2, "cider-05", Some(0)).unwrap();
    st.lock_in(1).unwrap(); st.lock_in(2).unwrap(); st.lock_in(3).unwrap();
    st.advance_beat().unwrap();
    st.play_reaction(1, "beer-08", 1).unwrap();          // Beer 8 − 1 = 7
    st.advance_beat().unwrap();
    st.resolve().unwrap();
    assert_eq!(st.players[0].hp, 14);
    assert_eq!(st.players[0].vessels[0].pulls_left, 7);
}

#[test]
fn test_reflect_sends_it_home_and_refuses_aoe() {
    // bob's Windfall (Damage 6) → alice; cara reflects with a hand-built
    // Wine vessel (the Plan F drain-test precedent for mid-game vessels).
    let mut st = at_lock();
    st.players[2].vessels.push(Vessel {
        deck: Deck::Wine, pulls_max: 6, pulls_left: 6, container: "glass".into(),
    });
    st.players[2].hand.push(crate::lc_cards::card_by_id("wine-08").unwrap());
    st.arm(2, "cider-04").unwrap();
    st.set_target(2, "cider-04", Some(0)).unwrap();
    st.lock_in(1).unwrap(); st.lock_in(2).unwrap(); st.lock_in(3).unwrap();
    st.advance_beat().unwrap();
    st.play_reaction(3, "wine-08", 1).unwrap();          // Wine 6 − 2 = 4
    st.advance_beat().unwrap();
    st.resolve().unwrap();
    assert_eq!(st.players[1].hp, 9);   // bob ate his own Windfall
    assert_eq!(st.players[0].hp, 15);

    // Reflect needs a seat target — an aoe play has none (I5):
    let mut st = at_lock();
    st.players[0].hand.push(crate::lc_cards::card_by_id("beer-05").unwrap());
    st.players[2].vessels.push(Vessel {
        deck: Deck::Wine, pulls_max: 6, pulls_left: 6, container: "glass".into(),
    });
    st.players[2].hand.push(crate::lc_cards::card_by_id("wine-08").unwrap());
    st.arm(1, "beer-05").unwrap();     // targets "all"
    st.lock_in(1).unwrap(); st.lock_in(2).unwrap(); st.lock_in(3).unwrap();
    st.advance_beat().unwrap();
    assert_eq!(st.play_reaction(3, "wine-08", 1), Err(LcError::BadTarget));
}

#[test]
fn test_two_cancels_resolve_lifo_and_the_second_fizzles() { // DDv2 §12
    let mut st = at_reveal();
    st.players[1].hand.push(crate::lc_cards::card_by_id("cider-08").unwrap());
    st.play_reaction(2, "cider-08", 1).unwrap();
    st.play_reaction(2, "cider-08", 1).unwrap();         // both legal, both paid
    assert_eq!(st.players[1].vessels[0].pulls_left, 6);  // 10 − 2 − 2
    st.advance_beat().unwrap();
    st.resolve().unwrap();
    assert_eq!(st.players[1].hp, 15);   // cancelled once; the echo fizzled
    assert_eq!(st.discards.len(), 3);   // the play + both reactions
}
```

- [ ] **Step 4: Commit**

```bash
git add drinkinggame/src/last_call.rs
git commit -m "feat(lastcall): play_reaction — the Reveal response window, LIFO cancel/reduce/reflect, TBD-7 structural"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

### Task 3: Engine — ghosts haunt

**Class:** B (logic, tests specified below)

**Why this class:** a pure transition and one term in the damage formula, every
case and expected value written here.

**Files:**
- Modify: `drinkinggame/src/last_call.rs`
- Test: `drinkinggame/src/last_call.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: Task 2's resolution fold; Plan F's `card_fx`; `HAUNT_BONUS`.
- Produces (exact — Tasks 4–5 build against these):

```rust
/// One ghost vote (DDv2 9.2): +HAUNT_BONUS onto an attack in flight.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Haunt {
    pub seat: usize, // the ghost
    pub play: u32,   // order_key of the ridden play
}
// LastCallState gains (container serde(default)):
//   pub haunts: Vec<Haunt>,
// PublicView gains the same field, projected verbatim (a vote is public).

// LcError gains:
//   NotAGhost,      // haunt by a living player — the mirror of NotAlive
//   AlreadyHaunted, // one vote per round (9.2)

pub fn haunt(&mut self, player_id: i64, order_key: u32) -> Result<(), LcError>;
```

The two new `LcError` variants compile everywhere without route edits — Plan
E's `map_lc` ends in `_ => 422` — Task 4 adds their explicit arms.

- [ ] **Step 1: `haunt`**

Guard order: `NotSeated` → `NotAGhost` (status must be `Eliminated` — the
only transition in the game reserved for the dead) → `WrongBeat` (beat must be
`Beat::Reveal`: "already in flight" is the window, I1) → `AlreadyHaunted`
(this seat already appears in `haunts` — the vec is per-round, so membership
*is* the once-per-round rule) → `BadTarget` (no play with that `order_key`,
or `card_fx(&play.card.id)` is not `Some(FxDef { op: EffectOp::Damage, .. })`
— votes ride attacks, nothing else).

Success: `haunts.push(Haunt { seat, play: order_key })`, `seq += 1`.

- [ ] **Step 2: The vote term in `resolve()`**

In Task 2's per-play damage formula the vote term goes live (I11): for a
non-cancelled Damage play, per subject `s`,
`max(0, magnitude + HAUNT_BONUS * votes(play) − reduce_total(play, s))` where
`votes(play)` counts `haunts` entries riding it. A cancelled play wastes its
votes (the ghost bet on a dead horse). At the end of the play loop, `clear()`
`haunts` alongside the reaction drain — eligibility refreshes with the round.
(Haunts are votes, not cards: nothing goes to `discards`.)

- [ ] **Step 3: Tests**

```rust
/// cara is a ghost; alice's beer-02 (Damage 4 → bob) is in flight, order_key 1.
fn ghost_table() -> LastCallState {
    let mut st = at_lock();
    st.players[2].status = Status::Eliminated;
    st.players[2].hand.clear(); // ghosts hold no cards (9.2)
    st.arm(1, "beer-02").unwrap();
    st.set_target(1, "beer-02", Some(1)).unwrap();
    st.lock_in(1).unwrap();
    st.lock_in(2).unwrap();
    st.advance_beat().unwrap(); // Lock → Reveal
    st
}
```

```rust
#[test]
fn test_a_ghost_haunts_once_a_round_for_plus_one() {
    let mut st = ghost_table();
    let seq = st.seq;
    st.haunt(3, 1).unwrap();
    assert_eq!(st.haunts, vec![Haunt { seat: 2, play: 1 }]);
    assert_eq!(st.seq, seq + 1);
    assert_eq!(st.haunt(3, 1), Err(LcError::AlreadyHaunted)); // one per round
    st.advance_beat().unwrap();
    st.resolve().unwrap();
    assert_eq!(st.players[1].hp, 10); // 15 − (4 + HAUNT_BONUS)
    assert!(st.haunts.is_empty());

    // The vote refreshes next round: walk round 2 to another reveal.
    for _ in 0..3 { st.advance_beat().unwrap(); } // Draw→Deal→Diplomacy→Lock
    st.arm(1, "beer-01").unwrap();                // Damage 2, still in hand
    st.set_target(1, "beer-01", Some(1)).unwrap();
    st.lock_in(1).unwrap();
    st.lock_in(2).unwrap();
    st.advance_beat().unwrap();                   // Reveal, order_key 1 again
    st.haunt(3, 1).unwrap();                      // fresh vote
}

/// MANDATORY — ghost-action legality: the living cannot haunt, and a vote
/// only rides a damage play inside the window.
#[test]
fn test_only_ghosts_haunt_and_only_attacks() {
    let mut st = ghost_table();
    assert_eq!(st.haunt(1, 1), Err(LcError::NotAGhost));   // alice lives
    assert_eq!(st.haunt(999, 1), Err(LcError::NotSeated));
    assert_eq!(st.haunt(3, 7), Err(LcError::BadTarget));   // no such play
    st.beat = Beat::Lock;
    assert_eq!(st.haunt(3, 1), Err(LcError::WrongBeat));   // window only

    // A heal in flight is not hauntable:
    let mut st = at_lock();
    st.players[2].status = Status::Eliminated;
    st.players[2].hand.clear();
    st.arm(1, "beer-03").unwrap(); // Buff, targets "self", Heal 2
    st.lock_in(1).unwrap();
    st.lock_in(2).unwrap();
    st.advance_beat().unwrap();
    assert_eq!(st.haunt(3, 1), Err(LcError::BadTarget));
}

#[test]
fn test_a_cancel_wastes_the_vote() { // I11 — the ghost bet on a dead horse
    let mut st = at_lock();
    st.players[2].status = Status::Eliminated;
    st.players[2].hand.clear();
    st.players[1].hand.push(crate::lc_cards::card_by_id("cider-08").unwrap());
    st.arm(1, "beer-02").unwrap();
    st.set_target(1, "beer-02", Some(1)).unwrap();
    st.lock_in(1).unwrap();
    st.lock_in(2).unwrap();
    st.advance_beat().unwrap();
    st.haunt(3, 1).unwrap();
    st.play_reaction(2, "cider-08", 1).unwrap();
    st.advance_beat().unwrap();
    st.resolve().unwrap();
    assert_eq!(st.players[1].hp, 15); // cancel kills the play, vote and all
}

#[test]
fn test_votes_and_reductions_share_one_ledger() { // I11's formula, both terms
    let mut st = at_lock();
    st.players[2].status = Status::Eliminated;
    st.players[2].hand.clear();
    st.players[0].hand.push(crate::lc_cards::card_by_id("beer-08").unwrap());
    st.arm(2, "cider-05").unwrap(); // Damage 4 → alice
    st.set_target(2, "cider-05", Some(0)).unwrap();
    st.lock_in(1).unwrap();
    st.lock_in(2).unwrap();
    st.advance_beat().unwrap();
    st.play_reaction(1, "beer-08", 1).unwrap(); // Reduce 3
    st.haunt(3, 1).unwrap();                    // +1
    st.advance_beat().unwrap();
    st.resolve().unwrap();
    assert_eq!(st.players[0].hp, 13); // 15 − max(0, 4 + 1 − 3)
}
```

- [ ] **Step 4: Commit**

```bash
git add drinkinggame/src/last_call.rs
git commit -m "feat(lastcall): ghosts haunt — one +1 vote per round onto an attack in flight"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

### Task 4: The react and haunt routes, and the window's grace

**Class:** C (auth-gated mutations under the room lock racing the ticker's
Reveal-deadline advance, plus a deadline write the beat clock also owns —
exactly the concurrency territory `plan-economics` names)

**Why this class:** the response routes and `lc_tick_room` contend for the same
room and the same `beat_deadline_ms`; "a reaction that loses the race gets a
clean 409, never a resolve-time surprise" and "the grace write happens under
the same guard the ticker re-checks under" are properties a reviewer must
check against the lock discipline, not properties a unit test can decide.

**Files:**
- Modify: `drinkinggame/src/lc_routes.rs` (two handlers, two forms,
  `extend_response_window`, `map_lc` arms; `#[cfg(test)]` unit test for the
  helper)
- Modify: `drinkinggame/src/routes.rs` (two route lines beside Plan E's)
- Test: `drinkinggame/tests/http.rs`

**Interfaces:**
- Consumes: Tasks 2–3's transitions; Plan E's `lc_lock`/`load_lc`/`LcCtx`,
  `persist_and_broadcast_lc`, `now_ms`, `map_lc`, the Task 1 handler skeleton;
  `GameError::{NotYourCall, OutOfTurn}`.
- Produces (exact — Task 5's JS posts these):

```rust
#[derive(Deserialize)] pub struct ReactForm { pub card_id: String, pub play: u32 }
#[derive(Deserialize)] pub struct HauntForm { pub play: u32 }

pub async fn lc_react_handler(State, PlayerSession, Path<String>, Form<ReactForm>) -> Response;
pub async fn lc_haunt_handler(State, PlayerSession, Path<String>, Form<HauntForm>) -> Response;

/// Decision I3: a public response keeps the window open at least
/// REACT_GRACE_SECS longer — never shortens it. Route-owned deadline data,
/// the arm_beat_clock precedent (E2); called only under the room guard, so
/// the ticker's relock-recheck sees the extended deadline or none at all.
pub(crate) fn extend_response_window(st: &mut LastCallState, now: i64);
pub(crate) const REACT_GRACE_SECS: u16 = 10;
```

| Method | Path | Body | Publishes |
| --- | --- | --- | --- |
| POST | `/room/{code}/lastcall/react` | `card_id`, `play` | full (`persist_and_broadcast_lc`) |
| POST | `/room/{code}/lastcall/haunt` | `play` | full |

Both are **full** publishes: a reaction and a vote are public events every
surface renders (the chips, the hand count, the extended timer) — "who is
subscribed and what are they looking at" answers the same way for both.
Neither route names a player anywhere (§6.1): the actor is the session, the
card is the actor's own, the play is a public order key.

- [ ] **Step 1: `extend_response_window` + its unit test**

```rust
pub(crate) fn extend_response_window(st: &mut LastCallState, now: i64) {
    if st.beat == Beat::Reveal {
        let floor = now + i64::from(REACT_GRACE_SECS) * 1000;
        st.beat_deadline_ms = Some(st.beat_deadline_ms.map_or(floor, |d| d.max(floor)));
    }
}
```

Unit test in `lc_routes.rs`'s tests module:

```rust
#[test]
fn test_extend_response_window_never_shortens() {
    let mut st = LastCallState::new(vec![(1, "a".into()), (2, "b".into())], 42);
    st.beat = Beat::Reveal;
    st.beat_deadline_ms = Some(1_000_000 + 3_000);   // 3s left
    extend_response_window(&mut st, 1_000_000);
    assert_eq!(st.beat_deadline_ms, Some(1_000_000 + 10_000)); // raised
    st.beat_deadline_ms = Some(1_000_000 + 15_000);  // 15s left
    extend_response_window(&mut st, 1_000_000);
    assert_eq!(st.beat_deadline_ms, Some(1_000_000 + 15_000)); // untouched
    st.beat = Beat::Lock;
    st.beat_deadline_ms = Some(7);
    extend_response_window(&mut st, 1_000_000);
    assert_eq!(st.beat_deadline_ms, Some(7)); // Reveal only
}
```

- [ ] **Step 2: The two handlers and `map_lc`**

Both are Plan E's Task 1 skeleton verbatim (lock → guard → `load_lc` →
transition → publish); the only differences: the transition call
(`ctx.st.play_reaction(player.id, &form.card_id, form.play)` /
`ctx.st.haunt(player.id, form.play)`), then on success
`extend_response_window(&mut ctx.st, now_ms());` and
`persist_and_broadcast_lc(&state, &ctx).await;` — the grace write and the
persist both happen under the guard, with no await between transition and
publish (the `broadcast_lc` discipline). Register both routes in `routes.rs`
beside Plan E's `/lastcall/lock` line.

`map_lc` gains the two variants in its existing buckets:

```rust
LcError::NotSeated | LcError::NotAlive | LcError::NotAGhost => {
    GameError::NotYourCall.into_response()
}
LcError::WrongBeat | LcError::AlreadyLocked | LcError::MustResolve
| LcError::AlreadyHaunted => GameError::OutOfTurn.into_response(),
```

(`WrongBeat` → 409 is the window's transport face: a reaction after the
window is "not now", not "never you".)

- [ ] **Step 3: http tests**

Rig, following Plan E Task 1's rig precedent (engine calls build the state;
`set_game_state` persists it): alice/Beer + bob/Cider vessels at Draw,
`cider-08` pushed into bob's hand, cara left Eliminated with a cleared hand
where the test needs a ghost, alice's `beer-02` armed→targeted(bob)→locked,
all lock, `advance_beat()` to Reveal (play in flight, order_key 1), then
`st.beat_deadline_ms = Some(now_ms() + 20_000)` and `set_game_state`.

```rust
#[tokio::test]
async fn test_lc_react_is_private_until_played_then_public() {
    // GET hand as alice: contains NEITHER "cider-08" NOR "Not So Fast"
    //   (bob's unplayed reaction is hand state — §3.3 over transport).
    // Open SSE, drain the snapshot.
    // POST react card_id=cider-08&play=1 as bob -> 204.
    // read_sse_until(&mut body, "event: lcpublic"): the frame contains
    //   "Not So Fast" — public exactly once played (I9). Full publish, not a
    //   bare tick: the segment contains "event: lcpublic".
}

#[tokio::test]
async fn test_lc_react_that_loses_the_race_gets_a_409() {
    // Rig with beat_deadline_ms = Some(now_ms() - 2_000). Open SSE (this puts
    // the room in active_rooms, so the 1 Hz ticker sees the expired window).
    // read_sse_until(&mut body, r#"data-beat="draw""#) — the chain collapsed
    // Reveal→Resolve→resolve() into round 2's Draw.
    // POST react card_id=cider-08&play=1 as bob -> 409 (WrongBeat→OutOfTurn):
    // the route relocked, reloaded, and the engine refused — the race has a
    // clean loser, never a post-resolution mutation.
}

#[tokio::test]
async fn test_a_response_extends_the_window() {
    // Rig with beat_deadline_ms = Some(now_ms() + 3_000); record it.
    // POST react as bob -> 204. Reload the game row (db::get_active_game) and
    // parse: beat_deadline_ms is strictly greater than the recorded value and
    // at least now_ms() + 8_000 (grace 10s minus slack) — decision I3 held
    // under the same guard the publish rode.
}

#[tokio::test]
async fn test_lc_haunt_is_for_ghosts_only_and_lands_public() {
    // Rig with cara Eliminated (hand cleared), member session for her.
    // POST haunt play=1 as alice (alive) -> 403 (NotAGhost→NotYourCall).
    // POST haunt play=1 as carol-the-non-member -> 403 (route guard).
    // POST haunt play=1 as cara -> 204; read_sse_until "event: lcpublic":
    //   the frame carries the haunt chip (Task 5's marker class or, until
    //   Task 5 lands, assert the seq moved via a fresh GET hand data-seq).
    // POST haunt play=1 as cara again -> 409 (AlreadyHaunted→OutOfTurn).
}
```

(If Task 5 has not landed when this task runs, the chip assertion in the last
test asserts on the projected `reactions`/`haunts` making the *next* lcpublic
frame — content-filtered, never positional; the executor notes which form ran.)

- [ ] **Step 4: Commit**

```bash
git add drinkinggame/src/lc_routes.rs drinkinggame/src/routes.rs drinkinggame/tests/http.rs
git commit -m "feat(lastcall): react and haunt routes — window-gated, guard-held, grace-extending"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

### Task 5: The response surfaces — hand-pane section, ghost bar, centre chips

**Class:** B (string builders with exact expected copy plus static
`node --check`-gated JS — the Plan E Task 4/5 precedent; nothing here locks,
gates or publishes)

**Files:**
- Modify: `drinkinggame/src/lc_render.rs` (`ActionBarView` fields,
  `lc_action_bar` ghost rows, `lc_screen_panel` centre chips; tests)
- Modify: `drinkinggame/src/lc_routes.rs` (`response_section_html`;
  `action_bar_view` fills the new fields; `hand_pane_html` appends the
  section and the ghost note)
- Modify: `drinkinggame/assets/lc_loop.js` (generic post-body builder)
- Modify: `drinkinggame/assets/lastcall.css` (response/chip rules — outside
  the reduced-motion block)
- Test: `drinkinggame/tests/http.rs`

**Interfaces:**
- Consumes: Task 2–3's `PublicView.reactions`/`PublicView.haunts` and
  projections; Task 4's routes; Plan E's `ActionBarView`, `lc_action_bar`,
  `targets_section_html` shape, `hand_pane_html`, `lc_loop.js`'s delegated
  `[data-lc-post]` listener and `note()`.
- Produces (exact):

```rust
// lc_render.rs — ActionBarView gains (route-side fill, never broadcast):
pub haunt_plays: Vec<(u32, String)>, // (order_key, "SRC → TGT"), damage plays only
pub haunted: bool,                   // this ghost already voted this round

// lc_routes.rs
fn response_section_html(st: &LastCallState, seat: usize) -> String;
```

- [ ] **Step 1: `response_section_html`**

Empty string unless `beat == Beat::Reveal`, the viewer is `Alive`, and the
viewer's hand holds ≥ 1 `CardKind::Reaction` card with ≥ 1 scope-legal play to
answer (I5's filter: "self" → subjects include the viewer; Reflect → the play
has a seat target). Otherwise, one block per holdable reaction
(`html_escape` on titles/names, the `targets_section_html` house style):

```html
<section class="lc-react"><h2>Response window</h2>
  <div class="lc-react-card" data-card-id="{id}">
    <span class="lc-react-title">{TITLE}</span>
    <button class="lc-btn lc-react-btn" data-lc-post="react"
            data-card-id="{id}" data-play="{order_key}">{VERB} {SRC} → {TGT}</button>
    <!-- one button per scope-legal play, in order_key order -->
  </div>
</section>
```

`{VERB}` from `card_rfx`: `Cancel` → `CANCEL`, `Reduce(_)` → `BLUNT`,
`Reflect` → `SEND BACK`. `{SRC}`/`{TGT}` are the play's source/target names
uppercased, `ALL` when the target is `None` (the E15 caption convention).
`hand_pane_html` appends it between the targets section and the actions
template — `#lc-hand` still leads, the seq gate and stale-drop are untouched.
When the viewer is `Eliminated`, `hand_pane_html` instead appends
`<p class="lc-ghost-note">GHOST — YOU HOLD NOTHING. THE TABLE STILL HEARS YOU.</p>`.

- [ ] **Step 2: The ghost action bar**

`action_bar_view` fills the new fields: `haunt_plays` = every play whose
`card_fx` op is `Damage`, as `(order_key, "{SRC} → {TGT}")` captions;
`haunted` = the viewer's seat appears in `st.haunts`. In `lc_action_bar`, the
`!alive` row (Plan E's precedence: after `outcome`/`!seated`, before beat)
becomes three rows:

| State | Thumb zone |
| --- | --- |
| `!alive`, beat Reveal, `!haunted`, `haunt_plays` non-empty | per play: `<button class="lc-btn lc-haunt-btn" data-lc-post="haunt" data-play="{k}">HAUNT {SRC} → {TGT} +1</button>` |
| `!alive`, beat Reveal, `haunted` | `<p class="lc-actions-hint">YOUR CURSE IS CAST</p>` |
| `!alive`, otherwise | `<p class="lc-actions-hint">YOU'RE OUT — HAUNT THE TABLE</p>` (Plan E's row, unchanged) |

- [ ] **Step 3: Centre chips on the big screen**

In `lc_screen_panel`'s `.lc-centre-play` (E15's block), after the `card_mini`:
when reactions/haunts ride this play,

```html
<div class="lc-centre-chips">
  <span class="lc-chip lc-chip-react" data-deck="{reaction deck}">{REACTOR}: {TITLE}</span>
  <!-- one per ReactionPlay answering this play, in played order -->
  <span class="lc-chip lc-chip-haunt">{GHOST} +1</span>
  <!-- one per Haunt riding this play -->
</div>
```

Names via `html_escape`, resolved from `view.seats`; deck as a class-feeding
`data-deck`, never hex. The mini table stays untouched (E15's split: the
phone's story is the bar and the hand pane). No `lc_mini_table` change.

- [ ] **Step 4: `lc_loop.js` — the generic body builder**

Replace the `data-vessel` special case in the delegated `[data-lc-post]`
click listener with one collector (this is the whole JS change — the
listeners, `note()`, posting and errors all exist):

```js
var body = [];
if (el.dataset.vessel) body.push("vessel=" + el.dataset.vessel);
if (el.dataset.cardId) body.push("card_id=" + encodeURIComponent(el.dataset.cardId));
if (el.dataset.play) body.push("play=" + el.dataset.play);
post(action, body.join("&"));
```

- [ ] **Step 5: CSS**, in the shell/screen sections of `lastcall.css` (tokens
exist, no new colours, nothing in the reduced-motion block):

```css
/* Plan I — the response window (decision I12). */
.lc-react { padding: 0 14px 10px; }
.lc-react-card { display: flex; flex-direction: column; gap: 6px; padding: 6px 0; }
.lc-react-title { font-family: var(--font-ui); font-weight: 700; font-size: 11px;
                  letter-spacing: .12em; text-transform: uppercase;
                  color: var(--lc-faint); }
.lc-ghost-note { padding: 10px 14px; font-family: var(--font-mono);
                 font-size: 11px; color: var(--lc-faint); }
.lc-centre-chips { display: flex; flex-direction: column; gap: 4px;
                   align-items: center; }
.lc-chip { font-family: var(--font-ui); font-weight: 700; font-size: 11px;
           letter-spacing: .1em; text-transform: uppercase;
           color: var(--lc-text); }
.lc-chip-haunt { color: var(--lc-rose); }
```

- [ ] **Step 6: Tests.** In `lc_render.rs` (fixtures built inline; every new
builder output joins the `no_hex` sweep):

```rust
#[test]
fn test_ghost_bar_haunts_only_in_the_window() {
    // ab(alive:false, beat: Reveal, haunt_plays: [(1, "ALICE → BOB")],
    //   haunted:false): contains r#"data-lc-post="haunt" data-play="1""#
    //   and "HAUNT ALICE → BOB +1".
    // haunted:true: contains "YOUR CURSE IS CAST", no data-lc-post="haunt".
    // beat Lock: contains "YOU'RE OUT — HAUNT THE TABLE" (Plan E's row and
    //   its existing test stay true).
    // alive:true at Reveal: no haunt button ever.
}

#[test]
fn test_centre_chips_ride_their_play() {
    // ring_fixture + one revealed play (order_key 1) + one ReactionPlay
    //   answering 1 + one Haunt on 1: panel contains "lc-chip-react",
    //   the reactor's name and the reaction title, and "{GHOST} +1" with
    //   lc-chip-haunt; a second play with no riders renders no
    //   lc-centre-chips block; no_hex.
}
```

In `http.rs`:

```rust
#[tokio::test]
async fn test_the_response_section_is_per_viewer() {
    // Task 4's rig (bob holds cider-08, play 1 in flight at Reveal).
    // GET hand as bob: contains "Response window", "CANCEL ALICE → BOB",
    //   r#"data-lc-post="react""#.
    // GET hand as alice (no reaction in hand): no "lc-react" at all.
    // Ghost cara: GET hand contains "GGHOST"-note copy exactly:
    //   "GHOST — YOU HOLD NOTHING. THE TABLE STILL HEARS YOU." and her
    //   actions template carries the HAUNT button for play 1.
}
```

- [ ] **Step 7: Commit**

```bash
git add drinkinggame/src/lc_render.rs drinkinggame/src/lc_routes.rs drinkinggame/assets/lc_loop.js drinkinggame/assets/lastcall.css drinkinggame/tests/http.rs
git commit -m "feat(lastcall): response surfaces — hand-pane window, ghost haunt bar, centre chips"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

## Browser checkpoint — after Task 5, before the final review

A human, a real focused browser, `cargo run -p drinkinggame`, two profiles, a
room, the big screen on a third window:

1. Play to a reveal with one phone holding a reaction (register Cider, keep
   cider-08 from a draw or rig it): the holder's hand pane shows the Response
   window section; the other phone shows nothing there — the window's
   existence looks identical on both (I2).
2. Press CANCEL: a chip lands under the centre play on the big screen within
   a second, the answered play resolves as nothing at the beat's end, and the
   timer visibly gains time when pressed near zero (I3).
3. Eliminate a player; at the next reveal their phone offers HAUNT … +1 in
   the thumb zone; press it — the rose +1 chip appears on the big screen, the
   target takes one extra, and a second press says "not now" via the note.
4. The ghost's HAND tab shows the ghost note, not an empty wheel error; their
   TABLE tab still spectates normally.

## Before the plan is done

- Tasks 1–3 and 5 are Class B (acceptance is the command; the tests are the
  spec); Task 4 is Class C and gets its per-task reviewer on a capable model.
  One whole-plan review of the branch diff at the end, on the most capable
  model. The reviewer brief must name: the I1 window vs `lc_advance_chain`
  (no chain edit — verify none crept in), the ticker/react race and its
  relock-recheck answer, the grace write sharing `beat_deadline_ms` with
  `arm_beat_clock`, §3.4.1/I9 (no new writer to `plays`; reactions public at
  creation), TBD-7's structural pin, both publishes staying await-free
  under the guard, and Plan J's public-only log: every loggable event this
  plan creates (`reactions`, `haunts`) is public at creation; an unplayed
  reaction must be unreachable from any log or broadcast path.
- No `cargo sqlx prepare` (no migration; two container-default fields only).
- Interfaces line up: Task 1's `card_rfx` is what Task 2's fold reads; Task
  2's `reactions` and Task 3's `haunts` are what Task 5's chips render and
  what Task 4's routes mutate; Task 4's form fields (`card_id`, `play`) are
  what Task 5's `data-*` attributes post.
- Every brief requirement maps to a task: window-only legality → Task 2
  (engine) + Task 4 (409 over transport); TBD-7 → Task 2; §3.4.1-shape
  secrecy → Task 2 (engine) + Task 4 (transport); ghost legality/once-per-
  round → Task 3; the F5 flip → Task 1; ghost UI → Task 5; the race → Task 4.
- STATUS updated: reactions and ghosts live; the hollow-systems list loses
  both; I2's dead-20s trade, I3's grace, and the no-reaction-flights deferral
  recorded as open decisions; `drinkinggame` still clippy-clean, distinct
  warnings still 17.

## Self-review (performed while writing)

- **Window design:** carved without touching E4's chain — Reveal is already a
  timed stop; checked `lc_advance_chain` needs no edit. The leak the brief
  warned about (window existence reveals holdings) is closed by I2's
  unconditional window and flagged for the user. The race is Task 4, Class C,
  pinned by `test_lc_react_that_loses_the_race_gets_a_409`.
- **Fx vocabulary:** no `EffectOp` added — `ReactionFx` is catalog-side with
  engine support in `resolve()` (counter/reduce/redirect all land in I11's
  one formula); F7's keyword predicates are untouched by construction.
- **§3.4.1:** no new writer to `plays`; the reaction path stores only at the
  moment of revelation; the mandatory JSON-absence test is in Task 2.
- **Ghosts:** grounded in the only design text that exists (§9.2 verbatim,
  +1, once per round, hold no cards, untargetable — the last two already
  enforced and named, not re-implemented); the smallness constraint is stated
  (amplify by 1, never create/target/drink).
- **Placeholder scan:** every route path, form field, guard order, copy
  string, constant, chip markup and test expectation is spelled out; prose
  only where a named pattern exists (E's handler skeleton, `targets_section_
  html`, the rig precedent).
- **Type consistency vs the Produces blocks:** `play_reaction`/`haunt`
  signatures match the `player_id: i64` house convention; `map_lc` buckets
  extended, not reshaped; `ActionBarView` extended, not forked; `Vessel`
  literal in tests matches the `three_man`-era field names used by Plan F's
  drain test; `ring_fixture()` gains `reactions: vec![], haunts: vec![]`
  (the known out-of-engine `PublicView` constructor — Task 2/3 executors add
  one line each, noted here so it is not a surprise compile error).
