# Last Call — Plan H: events and tabs (slice 4)

> **For agentic workers:** REQUIRED SUB-SKILLS: `plan-economics` (this repo's
> task classes, review policy and plan sizing) then
> superpowers:subagent-driven-development to execute task-by-task.

**Goal:** Build the two round-scoped content systems DDv2 §10.1–10.2 name and
leave hollow — public round events and private tab objectives — on the round
loop Plans D–F ship, sharing the Draw→Deal round-advance hook.

**Architecture:** Both systems follow Plan F's catalog pattern: static tables
in new modules (`lc_events.rs`, `lc_tabs.rs`), ids stored in the blob, rules
resolved from the binary by id lookup with unknown-id fail-soft. Selection is
**deterministic from `rng_seed` + round/seat — no RNG anywhere**: a stepped
cycle through a prime-sized table, so the engine stays pure and a replay from
the seed reproduces the night. Events are PUBLIC — a single
`Option<String>` on `LastCallState` (never two at once, by type), revealed at
the Draw→Deal edge, cleared by `resolve()`'s rollover, ridden on the existing
`lcpublic` banner swap. Tabs are PRIVATE — ids in the existing
`LcPlayer.tabs: Vec<String>`, never projected, rendered only in the viewer's
own hand fragment; completion is engine-detected inside `resolve()` and
announced by name only.

**Slice:** When this plan is done every round deals one public event that
actually bends that round (costs, targets, dots, end-of-round programs), every
player carries one secret objective that pays out in HP or pulls when the
engine detects it, settlements are announced without being explained, and the
phone/big screen both show it — all with zero new SSE events and zero new
routes. The LOG tab's content, the end-of-game reveal of unsettled tabs, and
pacts (§10.3, Plan G) are not this plan's.

**Execution order — binding:** This plan runs **after Plan E** (it retunes
E's `action_bar_view` charge line, appends to E's `hand_pane_html` assembly,
and its http tests drive E's `begin`/advance-chain flow), and therefore after
Plans C, D and F, whose Produces blocks it consumes — all four are unexecuted
as this is written, so every consumed signature below is copied from those
plans' Produces blocks, not from the tree. It is **independent of Plan G
(pacts)**: nothing here reads pact state, and `tab_def(id)`'s `Option` return
makes an unknown id in `tabs[]` inert — so if Plan G stores pact markers in
`tabs[]`, the two coexist; if G lands first and claims `tabs[]` outright,
Task 3's executor reconciles against the tree and reports the seam.

**Ledger:** `.superpowers/sdd/2026-08-11-last-call-plan-h-events-tabs/progress.md`
(gitignored).

---

## Proposed design decisions — awaiting user review

DDv2 gives events one sentence and a guardrail, tabs two sentences, and no
list, no vocabulary, no predicates for either (spec §9 records the gap).
Everything below is authored here. DDv1 §6 was mined for names and intent; its
mechanics were adapted, not copied, because half of them need systems that do
not exist. Every magnitude is table data a playtest can move.

- **H1 — Selection is a stepped cycle, not a hash-and-hope.**
  `event_for_round(seed, round)` = `EVENTS[(start + step·round) % 7]` with
  `start = seed % 7`, `step = seed % 6 + 1`. Seven is prime, so every step
  1–6 is coprime with it: the cycle visits **all seven events before any
  repeat and never deals the same event twice in a row** — "never two at
  once" and no-stutter, from arithmetic rather than memory. Same shape for
  tabs (H7). Pure computation from stored state; no RNG enters the engine.
- **H2 — Event storage is `LastCallState.event: Option<String>`.** At most
  one event is *representable*, so §10.1's "never two at once" is structural,
  the same move as §3.4/§6.1. Revealed at the Draw→Deal edge (DDv2 §5 beat 2:
  "Reveal this round's event, replacing the last"; §2.6: first event at
  round 1's beat 2, not at setup), cleared by the rollover — so an event
  lives Deal→Resolve of exactly its own round, and the Draw beat's banner
  strip is free for H10's settlement announcements. The walkthrough's
  visible-next-event is wrong per spec §9 and is not built: the schedule
  function could telegraph, and deliberately nothing calls it for
  `round + 1`.
- **H3 — Events are data + a hook enum the engine matches on.** `EventDef`
  carries `hook: EventHook` (seven variants, magnitudes as fields). The
  engine consults `lc_events::event_def(&id)` at its hook points — Plan F's
  `card_fx` pattern, including the fail-soft: an id the binary no longer
  knows resolves as no event.
- **H4 — The event list** (7 entries — timing is when the engine consults it):

  | id | Name | Timing | Effect (existing mechanics only) | Duration |
  | --- | --- | --- | --- | --- |
  | `happy-hour` | Happy Hour | cost sites (arm / lock / reveal charge / D4 ordering) | every card's pull charge is halved, rounded up (`pull_cost(...).div_ceil(2)`) | this round |
  | `last-orders` | Last Orders | `resolve()`, end-of-round program | every Alive player with no play this round takes 2 damage (`apply_damage`) | this round |
  | `toast` | Toast | applied once, at the Deal reveal | every Alive player drains 1 pull from their fullest vessel (`drain_pulls`) and heals 2 | instant |
  | `double-vision` | Double Vision | `resolve()`, subject computation | every hostile `one`-target play (fx op Damage / Dot / PullDrain) hits the next Alive seat clockwise from its target; heals and shields land where aimed | this round |
  | `big-shot` | Big Shot | `resolve()`, end-of-round program | the round's biggest spender (charged pulls > 0; ties: all of them) takes 2 damage | this round |
  | `house-pour` | House Pour | `resolve()`, dot tick | dots tick at double magnitude this resolve; expiry unchanged | this round |
  | `on-the-house` | On the House | `resolve()`, end-of-round program | every Alive player heals 2 | this round |

  Per-event guardrail check (§10.1: an event MUST NOT change how much anyone
  drinks beyond one extra pull): only `toast` touches pulls, by exactly 1;
  `happy-hour` only reduces drinking. Recorded here rather than tested — the
  guardrail is a design property of the table, not a predicate over it.
- **H5 — Events cut from DDv1's list, with reasons:** *Lights Up* (public
  hands would breach the §3.4 projection architecture this branch is built
  on); *Round of Shots* (needs the consenting-swap UI deferred with
  Diplomacy, D14); *Lock-In* (no reaction window exists to remove); *Bar
  Tab* (needs per-source damage attribution the engine doesn't keep —
  revisit with the LOG/ghost slice, which needs attribution anyway); *Two
  for One* (its race is decided at Draw, which now happens **before** the
  Deal reveal — the event would arrive after the decision it exists to
  provoke).
- **H6 — Event placement: a strip inside `#lc-banner`.** `lc_banner` gains a
  `.lc-event` child after the meta span. Both shells and the big screen
  already swap `#lc-banner` whole from the `lcpublic` frame's
  `template[data-lc-banner]` (and E10 put the timer inside the banner for
  exactly this atomicity), so one builder change covers phone F.1 and screen
  F.2 with no new JS and no new SSE. Suppressed when `outcome.is_some()` —
  the game-over banner owns that moment (E13).
- **H7 — Tab deal rule:** `tab_for(seed, seat, nth)` =
  `TABS[(seed % 7 + 3·seat + step·nth) % 7]`, `step = seed % 6 + 1`. The
  `3·seat` offset (3 coprime with 7) starts neighbouring seats on different
  tabs; `nth` (how many tabs the seat has been dealt before) steps a
  player's successive tabs so a replacement is never the tab just settled.
  Dealt at seating (`LastCallState::new` and `add_player` — DDv2 §2.6), and
  **at the Draw→Deal edge to any Alive player holding none** (DDv2 §5 beat
  2: "Deal a replacement tab to anyone who completed one"). The empty-refill
  rule also backfills version skew for free: an old blob's `tabs: []` player
  is simply dealt one at the next Deal.
- **H8 — `LcPlayer.tabs` stays `Vec<String>`** — ids into the static table,
  Plan F's id+lookup shape, for the same serde reason (`LastCallState`'s doc
  comment: nested structs stay strict; a richer nested type would either
  break old blobs or freeze stale rules into them). The vec holds the tabs
  *currently held* — normally exactly one; empty between a settlement and
  the next Deal. History lives in the container-level ledger (H10), not
  here. The active tab is `.last()`.
- **H9 — The tab list** (7 entries — every predicate is deck-fair: it is
  satisfiable by all five decks' catalogs, which is why no kind-based tab
  survived drafting — Soft has no Curse, Beer has no Util and no cost-3):

  | id | Name | Detection predicate (over engine state, at resolve) | Reward |
  | --- | --- | --- | --- |
  | `lie-low` | Lie Low | no play by this seat this round | +2 HP |
  | `showboat` | Showboat | 3+ plays by this seat this round | +2 pulls |
  | `bottoms-up` | Bottoms Up | `draws_this_round > 0` (used finish-&-draw) | +2 HP |
  | `high-roller` | High Roller | charged 4+ pulls on plays this round (post-handicap, event-aware) | +2 HP |
  | `deep-pockets` | Deep Pockets | ends the round holding 8+ cards | +2 pulls |
  | `cliffhanger` | Cliffhanger | ends the round Alive at 5 HP or less | +3 HP |
  | `peacemaker` | Peacemaker | played 1+ cards, none with fx op Damage / Dot / PullDrain | +2 HP |

  Rewards pay in HP and pulls, never in winning (DDv1 §6.2's rule, kept).
  `Hp(n)` heals with no ceiling (TBD-3); `Pulls(n)` refills the player's
  **most-depleted** vessel (tie → lowest index), capped at `pulls_max` — the
  inverse of F4's drain-the-fullest. Vendetta-class tabs (naming another
  player) are cut for now: they need target-void rules DDv1 §11 leaves open
  ("a dead target voids the tab") and a parametric deal — recorded, not
  built.
- **H10 — Detection runs inside `resolve()`,** as a step after the soft cap
  and before the outcome check / rollover, over a `round_plays` clone and a
  per-seat `spent` vector both captured at `resolve()` entry (immune to
  where the resolution program drains `plays`). Eliminated players are
  skipped — and elimination **voids** the victim's tabs (`tabs.clear()` in
  the elimination path; ghosts hold no objectives — the DDv1 §11 fallback).
  A settlement: pays the reward, removes the id from `tabs`, and pushes
  `TabSettle { seat, tab, round }` onto a new container-level
  `tab_ledger: Vec<TabSettle>` — the durable history Plan J's LOG and
  end-of-game reveal will read.
- **H11 — Completion IS announced, name only.** DDv2 §10.2: "announced after
  the fact, never before"; DDv1: "Sara settled a tab… never explains what it
  was." `PublicView` gains `settled: Vec<String>` — the *names* of seats
  with a ledger entry for the previous round — and the banner strip shows
  "{NAME} SETTLED A TAB" **during the Draw beat only**, the one beat with no
  event (H2 cleared it at rollover). The strip is therefore one thing at a
  time by construction: settlements at Draw, the event from Deal on. The tab
  id never enters `PublicView`.
- **H12 — The cost seam:** `happy-hour` centralizes charging in one new
  engine method, `effective_pull_cost(&self, cost, handicap_pct)`, used by
  `payment_plan`, the reveal charge, the D4 ordering total — and by a new
  `charged_pulls(&self, seat)` that Plan E's `action_bar_view` is retuned to
  call for its DRINK {n} chip (one named mechanical edit in `lc_routes.rs`),
  so the phone never tells a player to drink a different number than the
  engine debited. Lock-to-reveal consistency holds: only the rollover clears
  `event`, so the charge validated at lock is the charge applied at reveal
  (the D3 argument, extended).
- **H13 — Tabs render in the HAND pane,** as a `.lc-tabcard` section
  appended by `hand_pane_html` between E's targets section and the actions
  template. It rides the existing private fetch, its stale-drop, and §6.1's
  no-player-identifier route — a new private surface with zero new privacy
  machinery. The LOG tab stays empty (Plan J); the TABLE pane and big screen
  never show tabs.
- **H14 — Both catalogs hold 7 entries and the count is load-bearing** (H1's
  primality argument). A future entry-count change must revisit the
  coprimality of `step` (and `3·seat`) or the no-adjacent-repeat property
  silently dies; a test pins `EVENTS.len()`/`TABS.len()` at 7 and says why.
- **H15 (added post-review — controller-ruled, awaiting user eyes) — Betrayal
  is intent-based:** the aimed target decides the break; event redirection
  changes only where damage lands. Applies through Cancel: a cancelled play
  still betrays (ruled in Plan I's review).

---

## Global Constraints

Every task's requirements implicitly include this section.

### Scope

- Create: `drinkinggame/src/lc_events.rs`, `drinkinggame/src/lc_tabs.rs`
  (registered in `drinkinggame/src/lib.rs` beside `pub mod lc_cards;`).
- Modify: `drinkinggame/src/last_call.rs` (fields, Deal-edge work, resolve
  hooks, cost seam), `drinkinggame/src/lc_render.rs` (banner strip,
  `lc_tab_panel`, `ring_fixture` literals), `drinkinggame/src/lc_routes.rs`
  (two named mechanical edits: the `charged` line, the `hand_pane_html`
  append), `drinkinggame/assets/lastcall.css`, `drinkinggame/tests/http.rs`.
- **No new routes, no new SSE events, no migration, no template edits, no
  new JS** — the banner swap and the hand fetch already carry everything;
  `lc_loop.js` is untouched (nothing here needs a client-side behaviour).
  If a task finds itself registering a route or adding an SSE listener, it
  has crossed the line and must stop and report.

### Binding rules

- **Privacy invariants carried forward, all still in force:** tab identity
  never enters `PublicView` (extend the projection, never bypass it); the
  hand fragment stays the only surface that renders a tab; the private route
  keeps taking no player identifier (nothing here touches it); public
  renderers keep taking `&PublicView` (static catalog lookups by projected
  id are fine — they read the binary, not the state); renderers emit no hex
  (new builders join the `no_hex` sweep).
- **Serde version skew:** new `LastCallState` fields (`event`,
  `tab_ledger`) are container-level, covered by the existing
  `#[serde(default)]`. `TabSettle` is a new struct — strictness is moot, no
  old blob contains one. `LcPlayer` gains **no fields** (H8); `Card`, `Play`,
  `Effect` are untouched.
- **seq discipline:** the Deal-edge work and resolve additions run inside
  `advance_beat`/`resolve`, which already bump `seq` once per successful
  call — no additional bumps, no bump-free mutations.
- **No RNG in the engine** — H1/H7 are pure functions of stored state.
- Publish order, `broadcast_lc` await-freedom, frame filtering by content:
  unchanged and untouched (no `lc_routes` publish path is edited beyond the
  two named lines).
- Keep `drinkinggame` clippy-clean; the pre-existing distinct-warning count
  (17, all `drawingportfolio`) must not grow.

### Consumed interfaces (exact — from unexecuted Plans D/E/F's Produces)

```rust
// Plan D:
pub fn arm(&mut self, player_id: i64, card_id: &str) -> Result<(), LcError>;
pub fn lock_in(&mut self, player_id: i64) -> Result<(), LcError>;
pub fn advance_beat(&mut self) -> Result<(), LcError>;   // Draw→Deal edge = this plan's shared hook
pub fn resolve(&mut self) -> Result<(), LcError>;        // steps 1–7 as Plan D Task 5 numbers them
pub fn outcome(&self) -> Option<LcOutcome>;
fn payment_plan(player: &LcPlayer) -> Result<Vec<(usize, u8)>, LcError>; // grows a `halved: bool` param here
// apply_damage (private, both damage sites), at_lock()/seated() test fixtures,
// LastCallState.locked_plays, pull_cost(cost, handicap_pct).

// Plan F:
pub fn card_fx(id: &str) -> Option<FxDef>;               // hostile-op checks read this
pub struct FxDef { pub op: EffectOp, pub magnitude: i32, pub rounds: u32 }
pub enum EffectOp { Damage, Heal, Shield, Dot, PullDrain }
fn drain_pulls(player: &mut LcPlayer, n: i32);           // toast reuses it
// Real card ids/costs (beer-01 c1, beer-02 c2, beer-03 c1, beer-04 c2,
// cider-02 c1, cider-04 c3, soft-01 c1, soft-06 c1…) and the F6 openers.

// Plan E:
// lc_banner(view) with the E10 timer child; both shells + screen swap
//   #lc-banner from template[data-lc-banner] — H6 rides this.
// hand_pane_html -> {lc_hand_pane(...)}{targets_section}{<template data-lc-actions>…}
// action_bar_view(st, player_id).charged  — the line H12 retunes.
// POST …/lastcall/begin + lc_advance_chain — the http tests drive them.
```

### Verification

**Verification for every task:** `./scripts/verify.sh` — all green, output
quoted in the report. Never a bare `cargo test`.

**Baseline before Task 1:** whatever Plan E's ledger records at its close
(the pre-C/D figure was 371 tests; the invariants are *verify green* and *17
distinct clippy warnings, `drinkinggame` clean*, not a fixed count — read the
number from `.superpowers/sdd/2026-08-11-last-call-plan-e-loop-wiring/progress.md`
and record it in this plan's ledger before starting).

**Browser checkpoint: one**, after Task 5, before the final review. **No
`cargo sqlx prepare`** (no migration; `drinkinggame` is runtime-checked).

---

### Task 1: The two catalogs — `lc_events.rs` and `lc_tabs.rs`

**Class:** B (logic, tests specified below)

**Why this class:** static data plus pure selection arithmetic; every
expected value is written out, including the pinned deal sequences.

**Files:**
- Create: `drinkinggame/src/lc_events.rs`
- Create: `drinkinggame/src/lc_tabs.rs`
- Modify: `drinkinggame/src/lib.rs` (two `pub mod` lines beside `lc_cards`)
- Test: both new modules' `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: `Play`, `LcPlayer`, `Status` from `last_call.rs`;
  `lc_cards::card_fx`, `EffectOp` (Plan F).
- Produces (exact — Tasks 2–5 build against these):

```rust
// lc_events.rs
pub const EVENT_COUNT: usize = 7; // prime — H1/H14; a test names the argument

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventHook {
    CostHalf,                        // happy-hour
    NoPlayPenalty { dmg: i32 },      // last-orders
    Toast { drain: i32, heal: i32 }, // toast — applied once at the Deal reveal
    HostileRedirect,                 // double-vision
    TopSpenderHit { dmg: i32 },      // big-shot
    DotBoost { mult: i32 },          // house-pour
    TableHeal { heal: i32 },         // on-the-house
}

pub struct EventDef {
    pub id: &'static str,
    pub title: &'static str, // UPPERCASE display name
    pub text: &'static str,  // one sentence of rules the table reads aloud
    pub hook: EventHook,
}

pub const EVENTS: [EventDef; EVENT_COUNT];
pub fn event_def(id: &str) -> Option<&'static EventDef>; // None: fail-soft (H3)
pub fn event_for_round(seed: u64, round: u32) -> &'static EventDef;

// lc_tabs.rs
pub const TAB_COUNT: usize = 7; // prime — same argument

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TabCheck {
    NoPlays,            // lie-low
    PlaysAtLeast(u8),   // showboat (3)
    FinishedVessel,     // bottoms-up
    SpentAtLeast(u8),   // high-roller (4)
    HandAtLeast(usize), // deep-pockets (8)
    HpAtMost(i32),      // cliffhanger (5)
    NoHostilePlays,     // peacemaker: 1+ plays, none Damage/Dot/PullDrain
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TabReward { Hp(i32), Pulls(u8) }

pub struct TabDef {
    pub id: &'static str,
    pub title: &'static str,
    pub text: &'static str,
    pub check: TabCheck,
    pub reward: TabReward,
}

pub const TABS: [TabDef; TAB_COUNT];
pub fn tab_def(id: &str) -> Option<&'static TabDef>;
pub fn tab_for(seed: u64, seat: usize, nth: usize) -> &'static TabDef;

/// Pure predicate: did `seat` meet `check` this round? `round_plays` is the
/// round's play list captured before resolution drains it; `spent` is the
/// seat's charged pulls (event-aware); `player` is the post-resolution
/// player. Hostility is read from `card_fx` — catalog truth, not blob truth.
pub fn tab_met(
    check: &TabCheck,
    seat: usize,
    round_plays: &[Play],
    player: &LcPlayer,
    spent: u8,
) -> bool;
```

- [ ] **Step 1: `lc_events.rs`**

Module doc: the H1 selection argument (prime table, coprime step, full cycle,
no adjacent repeat), the H3 fail-soft, the §10.1 guardrail note per H4. The
selection function, verbatim:

```rust
pub fn event_for_round(seed: u64, round: u32) -> &'static EventDef {
    let start = (seed % EVENT_COUNT as u64) as usize;
    // 1..=6, all coprime with 7: the cycle hits all seven events before any
    // repeat and never deals the same event in adjacent rounds (H1).
    let step = (seed % (EVENT_COUNT as u64 - 1)) as usize + 1;
    &EVENTS[(start + step * round as usize) % EVENT_COUNT]
}
```

`EVENTS` in this exact order (the order is load-bearing — Task 2's pinned
sequences index into it):

```rust
pub const EVENTS: [EventDef; EVENT_COUNT] = [
    EventDef { id: "happy-hour", title: "HAPPY HOUR", hook: EventHook::CostHalf,
        text: "Every card costs half its pulls this round, rounded up." },
    EventDef { id: "last-orders", title: "LAST ORDERS", hook: EventHook::NoPlayPenalty { dmg: 2 },
        text: "Play at least one card this round or take 2 damage. No sitting this one out." },
    EventDef { id: "toast", title: "TOAST", hook: EventHook::Toast { drain: 1, heal: 2 },
        text: "Everyone drinks 1 pull together and heals 2. The only moment nobody's competing." },
    EventDef { id: "double-vision", title: "DOUBLE VISION", hook: EventHook::HostileRedirect,
        text: "Every attack hits the player left of its target this round. Aim wrong on purpose." },
    EventDef { id: "big-shot", title: "BIG SHOT", hook: EventHook::TopSpenderHit { dmg: 2 },
        text: "The round's biggest spender takes 2 damage. Nobody likes a show-off." },
    EventDef { id: "house-pour", title: "HOUSE POUR", hook: EventHook::DotBoost { mult: 2 },
        text: "Every curse ticks double this round. Old grudges, freshly poured." },
    EventDef { id: "on-the-house", title: "ON THE HOUSE", hook: EventHook::TableHeal { heal: 2 },
        text: "Everyone heals 2 at the end of the round. The house is feeling generous." },
];
```

`event_def` = `EVENTS.iter().find(|e| e.id == id)`.

- [ ] **Step 2: `lc_tabs.rs`**

Module doc: H7's deal rule and why `3·seat` (coprime spread), H8's
current-tabs-only semantics, the deck-fairness rule from H9 (kind-based
predicates were cut because Soft has no Curse, Beer no Util and no cost-3).
The deal function, verbatim:

```rust
pub fn tab_for(seed: u64, seat: usize, nth: usize) -> &'static TabDef {
    // 3 is coprime with 7: neighbouring seats start on different tabs.
    let start = (seed % TAB_COUNT as u64) as usize + seat * 3;
    // 1..=6, coprime with 7: a replacement is never the tab just settled.
    let step = (seed % (TAB_COUNT as u64 - 1)) as usize + 1;
    &TABS[(start + step * nth) % TAB_COUNT]
}
```

`TABS` in this exact order (also load-bearing for the pinned deals):

```rust
pub const TABS: [TabDef; TAB_COUNT] = [
    TabDef { id: "lie-low", title: "LIE LOW", check: TabCheck::NoPlays,
        reward: TabReward::Hp(2),
        text: "Play no card for a whole round. Look innocent." },
    TabDef { id: "showboat", title: "SHOWBOAT", check: TabCheck::PlaysAtLeast(3),
        reward: TabReward::Pulls(2),
        text: "Play three or more cards in one round. Make it look easy." },
    TabDef { id: "bottoms-up", title: "BOTTOMS UP", check: TabCheck::FinishedVessel,
        reward: TabReward::Hp(2),
        text: "Finish a vessel and draw. The tab covers the next one." },
    TabDef { id: "high-roller", title: "HIGH ROLLER", check: TabCheck::SpentAtLeast(4),
        reward: TabReward::Hp(2),
        text: "Spend four or more pulls on plays in a single round." },
    TabDef { id: "deep-pockets", title: "DEEP POCKETS", check: TabCheck::HandAtLeast(8),
        reward: TabReward::Pulls(2),
        text: "End a round holding eight or more cards. Hoard politely." },
    TabDef { id: "cliffhanger", title: "CLIFFHANGER", check: TabCheck::HpAtMost(5),
        reward: TabReward::Hp(3),
        text: "End a round alive on 5 HP or less. Live dangerously." },
    TabDef { id: "peacemaker", title: "PEACEMAKER", check: TabCheck::NoHostilePlays,
        reward: TabReward::Hp(2),
        text: "Play at least one card in a round without hurting anyone." },
];
```

`tab_met`, with hostility defined once:

```rust
pub fn tab_met(
    check: &TabCheck,
    seat: usize,
    round_plays: &[Play],
    player: &LcPlayer,
    spent: u8,
) -> bool {
    let mine = || round_plays.iter().filter(|p| p.source_seat == seat);
    let hostile = |p: &&Play| {
        crate::lc_cards::card_fx(&p.card.id).is_some_and(|f| {
            matches!(f.op, EffectOp::Damage | EffectOp::Dot | EffectOp::PullDrain)
        })
    };
    match check {
        TabCheck::NoPlays => mine().count() == 0,
        TabCheck::PlaysAtLeast(n) => mine().count() >= *n as usize,
        TabCheck::FinishedVessel => player.draws_this_round > 0,
        TabCheck::SpentAtLeast(n) => spent >= *n,
        TabCheck::HandAtLeast(n) => player.hand.len() >= *n,
        TabCheck::HpAtMost(n) => player.hp <= *n,
        TabCheck::NoHostilePlays => mine().count() >= 1 && !mine().any(|p| hostile(&p)),
    }
}
```

- [ ] **Step 3: Tests** (in each module's tests block; expected values final —
the seed-42 arithmetic is `start 0, step 1` for events and `start 3·seat,
step 1` for tabs, hand-checkable from the formulas):

```rust
// lc_events.rs
#[test]
fn test_event_selection_is_pinned_and_cycles() {
    // seed 42: start 0, step 1 — rounds 1..=7 walk the table in order.
    let ids: Vec<&str> = (1..=7).map(|r| event_for_round(42, r).id).collect();
    assert_eq!(ids, vec!["last-orders", "toast", "double-vision", "big-shot",
                         "house-pour", "on-the-house", "happy-hour"]);
    // seed 0xC0FFEE: start 4, step 5 — a second, non-trivial pin.
    assert_eq!(event_for_round(0xC0FFEE, 1).id, "toast");        // (4+5)%7  = 2
    assert_eq!(event_for_round(0xC0FFEE, 2).id, "happy-hour");   // (4+10)%7 = 0
    assert_eq!(event_for_round(0xC0FFEE, 3).id, "house-pour");   // (4+15)%7 = 5
}

#[test]
fn test_no_event_repeats_back_to_back() { // H1
    for seed in 0..50u64 {
        for round in 1..30u32 {
            assert_ne!(
                event_for_round(seed, round).id,
                event_for_round(seed, round + 1).id,
                "seed={seed} round={round}"
            );
        }
    }
}

#[test]
fn test_event_catalog_shape() {
    assert_eq!(EVENTS.len(), 7); // prime — H14: changing this count breaks
                                 // the coprimality argument; revisit H1 first.
    let ids: std::collections::HashSet<&str> = EVENTS.iter().map(|e| e.id).collect();
    assert_eq!(ids.len(), EVENTS.len());
    assert!(EVENTS.iter().all(|e| !e.title.is_empty() && !e.text.is_empty()));
    assert_eq!(event_def("happy-hour").unwrap().hook, EventHook::CostHalf);
    assert!(event_def("nope").is_none()); // the fail-soft arm exists (H3)
}

// lc_tabs.rs
#[test]
fn test_tab_deal_is_pinned_per_seat_and_nth() { // H7, seed 42: start 3·seat, step 1
    assert_eq!(tab_for(42, 0, 0).id, "lie-low");     // 0
    assert_eq!(tab_for(42, 1, 0).id, "high-roller"); // 3
    assert_eq!(tab_for(42, 2, 0).id, "peacemaker");  // 6
    assert_eq!(tab_for(42, 3, 0).id, "bottoms-up");  // 9 % 7 = 2
    assert_eq!(tab_for(42, 0, 1).id, "showboat");    // alice's replacement ≠ lie-low
    for seed in 0..50u64 {
        for nth in 0..10 {
            assert_ne!(tab_for(seed, 2, nth).id, tab_for(seed, 2, nth + 1).id);
        }
    }
}

#[test]
fn test_tab_catalog_shape() {
    assert_eq!(TABS.len(), 7); // prime — same H14 note as EVENTS
    let ids: std::collections::HashSet<&str> = TABS.iter().map(|t| t.id).collect();
    assert_eq!(ids.len(), TABS.len());
    for t in TABS.iter() {
        match t.reward {
            TabReward::Hp(n) => assert!((1..=5).contains(&n), "{}", t.id),
            TabReward::Pulls(n) => assert!((1..=4).contains(&n), "{}", t.id),
        }
    }
    assert!(tab_def("nope").is_none());
}

#[test]
fn test_tab_met_predicates() {
    // Fixture: a bare LcPlayer (hand of 3, hp 15, draws_this_round 0) and a
    // round_plays of two plays by seat 0 — beer-01 (Damage) and soft-01
    // (Heal) — built inline from lc_cards::card_by_id.
    // NoPlays: false for seat 0, true for seat 1.
    // PlaysAtLeast(3): false at 2 plays; PlaysAtLeast(2): true.
    // FinishedVessel: false; true after draws_this_round = 5.
    // SpentAtLeast(4): spent 3 -> false, spent 4 -> true.
    // HandAtLeast(8): false at 3; true after hand grows to 8.
    // HpAtMost(5): false at 15; true at 5 and at 3.
    // NoHostilePlays: false for seat 0 (beer-01 is Damage); true for a
    //   round_plays holding only soft-01; false for a seat with no plays.
}
```

- [ ] **Step 4: Commit**

```bash
git add drinkinggame/src/lc_events.rs drinkinggame/src/lc_tabs.rs drinkinggame/src/lib.rs
git commit -m "feat(lastcall): event and tab catalogs — 7+7 entries, deterministic stepped-cycle selection"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

### Task 2: Events in the engine — reveal, hooks, the cost seam

**Class:** B (logic, tests specified below)

**Why this class:** pure state-machine edits with every hook's expected
values written out; the one route-file edit is a named one-line retune whose
correctness the DRINK-parity test encodes.

**Files:**
- Modify: `drinkinggame/src/last_call.rs`
- Modify: `drinkinggame/src/lc_routes.rs` (the `action_bar_view` `charged`
  line only — H12)
- Modify: `drinkinggame/src/lc_render.rs` (`ring_fixture()`: the `PublicView`
  literal gains `event: None,`)
- Test: `drinkinggame/src/last_call.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: Task 1's `EVENTS`/`event_def`/`event_for_round`/`EventHook`;
  Plan D's `advance_beat`/`resolve` structure and `apply_damage`; Plan F's
  `drain_pulls`, `card_fx`; `pull_cost`.
- Produces (exact — Tasks 3–5 and the retuned route line build on these):

```rust
// LastCallState gains (container #[serde(default)] covers it):
/// The round's revealed event id, or None during Draw / before round 1's
/// Deal / after a rollover. At most one event is representable — DDv2
/// §10.1's "never two at once" is this type (H2).
pub event: Option<String>,

// PublicView gains, projected verbatim:
pub event: Option<String>,

/// pull_cost with the active event applied — the ONLY charging entry point
/// from here on (H12). happy-hour halves the charged pulls, rounded up.
pub fn effective_pull_cost(&self, cost: u8, handicap_pct: u16) -> u8;

/// The seat's total charged pulls over its plays in `self.plays` —
/// event-aware. Plan E's DRINK chip and Task 3's SpentAtLeast read this.
pub fn charged_pulls(&self, seat: usize) -> u8;
```

- [ ] **Step 1: field, projection, cost seam**

Add `event` to `LastCallState` and `PublicView`; project it verbatim in
`public_view()`; add `event: None,` to `ring_fixture()`'s literal (the only
out-of-engine `PublicView` constructor, the Plan D Task 1 precedent).

```rust
pub fn effective_pull_cost(&self, cost: u8, handicap_pct: u16) -> u8 {
    let base = pull_cost(cost, handicap_pct);
    match self.event.as_deref().and_then(crate::lc_events::event_def) {
        Some(e) if e.hook == crate::lc_events::EventHook::CostHalf => base.div_ceil(2),
        _ => base,
    }
}

pub fn charged_pulls(&self, seat: usize) -> u8 {
    let h = self.players.get(seat).map_or(100, |p| p.handicap_pct);
    self.plays
        .iter()
        .filter(|p| p.source_seat == seat)
        .map(|p| self.effective_pull_cost(p.card.cost, h))
        .sum()
}
```

Thread the event through the charging sites: `payment_plan` gains a
`halved: bool` parameter (its three call sites — arm, `lock_in`, the reveal
charge — pass `matches!(… CostHalf)` computed once from `self.event`); the
D4 ordering total in the reveal switches from `pull_cost` to
`effective_pull_cost`. Lock→reveal consistency is free: only the rollover
clears `event`, and reveal precedes resolve — document it at the
`payment_plan` `expect`.

In `lc_routes.rs`, retune `action_bar_view`'s `charged` computation to
`st.charged_pulls(seat)` (H12 — one line; if Plan E's executed shape named it
differently, adjust to the tree and note it in the report).

- [ ] **Step 2: the Deal reveal and the rollover clear**

In `advance_beat`'s Draw→Deal arm (which today clears `drawing` — Plan D's
"no extra work" note for the other edges stands):

```rust
// DDv2 §5 beat 2: reveal this round's event, replacing the last; §2.6:
// the first is dealt here at round 1, never at setup. Deterministic from
// the stored seed (H1) — no RNG enters the engine.
let def = crate::lc_events::event_for_round(self.rng_seed, self.round);
self.event = Some(def.id.to_string());
if let crate::lc_events::EventHook::Toast { drain, heal } = def.hook {
    for p in self.players.iter_mut().filter(|p| p.status == Status::Alive) {
        drain_pulls(p, drain); // fullest vessel, F4's helper
        p.hp += heal;          // no ceiling (TBD-3)
    }
}
```

In `resolve()`'s rollover (step 7): `self.event = None;` — the event lives
Deal→Resolve of exactly its round (H2). The game-over freeze (D16) skips the
rollover, so `event` may stay `Some` in the final tableau; Task 4's builder
suppresses it under `outcome`.

- [ ] **Step 3: the resolve hooks**

Resolve the active hook once at `resolve()` entry
(`let hook = self.event.as_deref().and_then(event_def).map(|e| e.hook);`),
alongside a per-seat spend snapshot for Big Shot (and Task 3):
`let spent: Vec<u8> = (0..self.players.len()).map(|s| self.charged_pulls(s)).collect();`
— captured before step 1 drains anything. Then:

- **Step 1 (subjects), `HostileRedirect`:** for a `targets == "one"` play
  whose `card_fx` op is `Damage`/`Dot`/`PullDrain`, the subject becomes the
  next Alive seat clockwise from the play's target — `(target + k) % n` for
  the smallest `k ≥ 1` landing on an Alive seat, falling back to the target
  itself if no other seat is Alive. Heals/shields land where aimed (H4).
- **Step 2 (dot ticks), `DotBoost { mult }`:** each tick applies
  `magnitude * mult`; stored magnitude and expiry untouched.
- **New step 2.5 — the end-of-round program**, after dots, before queued
  effects are appended:
  - `NoPlayPenalty { dmg }`: every Alive player with no play in the round's
    plays takes `dmg` via `apply_damage`.
  - `TopSpenderHit { dmg }`: every Alive seat whose snapshot `spent` equals
    the maximum and is > 0 takes `dmg` (ties: all of them — H4).
  - `TableHeal { heal }`: every Alive player heals `heal`.

- [ ] **Step 4: Tests** (fixtures: Plan D's `at_lock()` — seed 42, alice
Beer / bob Cider / cara Soft with F6 openers; `at_lock` sets `beat` directly,
so `event` stays `None` unless a test sets it — which is how each hook is
tested in isolation):

```rust
#[test]
fn test_the_event_lives_deal_to_resolve() { // H2 — and "never two at once"
    let mut st = seated();
    st.set_vessel(1, Deck::Beer, "can").unwrap();
    st.set_vessel(2, Deck::Cider, "bottle").unwrap();
    st.round = 2; // round 1's Draw is the lobby; use a plain round
    assert_eq!(st.event, None);            // Draw: no event
    st.advance_beat().unwrap();            // Draw -> Deal: the reveal
    assert_eq!(st.event.as_deref(), Some("toast")); // seed 42, round 2 (Task 1 pin)
    assert_eq!(st.public_view().event.as_deref(), Some("toast"));
    for _ in 0..4 { st.advance_beat().unwrap(); } // ... -> Resolve
    assert_eq!(st.event.as_deref(), Some("toast")); // still the same ONE event
    st.resolve().unwrap();
    assert_eq!(st.event, None);            // rollover cleared it
    st.advance_beat().unwrap();            // round 3's Deal
    assert_eq!(st.event.as_deref(), Some("double-vision")); // replaced, never two
}

#[test]
fn test_toast_pours_one_and_heals_two() { // H4, at the Deal reveal
    let mut st = seated();
    st.set_vessel(1, Deck::Beer, "can").unwrap();   // 8 pulls
    st.set_vessel(2, Deck::Cider, "bottle").unwrap(); // 10
    st.set_vessel(3, Deck::Soft, "glass").unwrap();  // 6
    st.round = 2;
    st.advance_beat().unwrap(); // reveals toast (seed 42 round 2)
    assert_eq!(st.players[0].vessels[0].pulls_left, 7);
    assert_eq!(st.players[1].vessels[0].pulls_left, 9);
    assert_eq!(st.players[2].vessels[0].pulls_left, 5);
    assert!(st.players.iter().all(|p| p.hp == 17));
}

#[test]
fn test_happy_hour_halves_the_charge_and_the_chip_agrees() { // H4, H12
    let mut st = at_lock();
    st.event = Some("happy-hour".into());
    st.arm(1, "beer-02").unwrap(); // cost 2 -> pull_cost 2 -> halved 1
    st.set_target(1, "beer-02", Some(1)).unwrap();
    st.lock_in(1).unwrap();
    st.advance_beat().unwrap(); // the reveal charges
    assert_eq!(st.players[0].vessels[0].pulls_left, 7); // 8 - 1, not 8 - 2
    assert_eq!(st.charged_pulls(0), 1); // what the DRINK chip will show
    assert_eq!(st.effective_pull_cost(3, 150), 3); // ceil(ceil(3*1.5)/2) = ceil(5/2)
}

#[test]
fn test_last_orders_charges_the_silent() { // H4
    let mut st = at_lock();
    st.event = Some("last-orders".into());
    st.arm(1, "beer-01").unwrap();
    st.set_target(1, "beer-01", Some(1)).unwrap();
    st.lock_in(1).unwrap();
    st.advance_beat().unwrap();
    st.advance_beat().unwrap();
    st.resolve().unwrap();
    assert_eq!(st.players[0].hp, 15); // played: exempt
    assert_eq!(st.players[1].hp, 11); // 15 - 2 (beer-01) - 2 (penalty)
    assert_eq!(st.players[2].hp, 13); // 15 - 2 (penalty)
}

#[test]
fn test_double_vision_redirects_attacks_not_heals() { // H4
    let mut st = at_lock();
    st.event = Some("double-vision".into());
    st.arm(1, "beer-01").unwrap(); // Damage 2, aimed at bob (seat 1)
    st.set_target(1, "beer-01", Some(1)).unwrap();
    st.lock_in(1).unwrap();
    st.arm(3, "soft-01").unwrap(); // Heal 2, aimed at alice (seat 0)
    st.set_target(3, "soft-01", Some(0)).unwrap();
    st.lock_in(3).unwrap();
    st.advance_beat().unwrap();
    st.advance_beat().unwrap();
    st.resolve().unwrap();
    assert_eq!(st.players[1].hp, 15); // aimed at, missed
    assert_eq!(st.players[2].hp, 13); // seat left of the target took it
    assert_eq!(st.players[0].hp, 17); // the heal landed where aimed
}

#[test]
fn test_big_shot_taxes_the_top_spender() { // H4
    let mut st = at_lock();
    st.event = Some("big-shot".into());
    st.arm(1, "beer-02").unwrap(); // 2 pulls — the big spender
    st.set_target(1, "beer-02", Some(2)).unwrap();
    st.lock_in(1).unwrap();
    st.arm(2, "cider-02").unwrap(); // 1 pull (drain -> cara)
    st.set_target(2, "cider-02", Some(2)).unwrap();
    st.lock_in(2).unwrap();
    st.advance_beat().unwrap();
    st.advance_beat().unwrap();
    st.resolve().unwrap();
    assert_eq!(st.players[0].hp, 13); // top spender taxed
    assert_eq!(st.players[1].hp, 15); // underbidder untouched
    assert_eq!(st.players[2].hp, 11); // beer-02's 4, plus nothing else
}

#[test]
fn test_house_pour_doubles_the_tick_this_round_only() { // H4
    let mut st = at_lock();
    st.effects.push(Effect {
        source_play: 0, subject: 0, op: EffectOp::Dot,
        magnitude: 1, expires_round: 9,
    });
    st.event = Some("house-pour".into());
    st.advance_beat().unwrap();
    st.advance_beat().unwrap();
    st.resolve().unwrap();
    assert_eq!(st.players[0].hp, 13); // 15 - 1*2 — and event now cleared
    for _ in 0..5 { st.advance_beat().unwrap(); } // next round's event is
    st.event = None;                              // forced off for isolation
    st.resolve().unwrap();
    assert_eq!(st.players[0].hp, 12); // back to 1 per tick
}

#[test]
fn test_on_the_house_heals_the_table() { // H4
    let mut st = at_lock();
    st.event = Some("on-the-house".into());
    st.advance_beat().unwrap();
    st.advance_beat().unwrap();
    st.resolve().unwrap();
    assert!(st.players.iter().all(|p| p.hp == 17));
}

#[test]
fn test_an_unknown_event_id_is_inert() { // H3's fail-soft
    let mut st = at_lock();
    st.event = Some("closing-time".into()); // an id this binary never knew
    st.advance_beat().unwrap();
    st.advance_beat().unwrap();
    st.resolve().unwrap(); // no panic, no hook fired
    assert!(st.players.iter().all(|p| p.hp == 15));
}
```

- [ ] **Step 5: Commit**

```bash
git add drinkinggame/src/last_call.rs drinkinggame/src/lc_routes.rs drinkinggame/src/lc_render.rs
git commit -m "feat(lastcall): events in the engine — Deal reveal, seven hooks, the happy-hour cost seam"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

### Task 3: Tabs in the engine — deal, detect, settle, void

**Class:** B (logic, tests specified below — including the secrecy test,
which is encodable as a JSON-absence assertion and is therefore a
test-is-the-spec case, the Plan D Task 3 precedent)

**Files:**
- Modify: `drinkinggame/src/last_call.rs`
- Modify: `drinkinggame/src/lc_render.rs` (`ring_fixture()`: the `PublicView`
  literal gains `settled: Vec::new(),`)
- Test: `drinkinggame/src/last_call.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: Task 1's `tab_for`/`tab_def`/`tab_met`/`TabReward`; Task 2's
  `charged_pulls` snapshot pattern; Plan D's `resolve()` step numbering and
  elimination path.
- Produces (exact — Tasks 4–5 and Plan J build against these):

```rust
/// One settled tab — the durable history Plan J's LOG and end-of-game
/// reveal read. New struct: serde strictness is moot, no old blob has one.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct TabSettle {
    pub seat: usize,
    pub tab: String,
    pub round: u32, // the round it settled in
}

// LastCallState gains (container #[serde(default)] covers it):
pub tab_ledger: Vec<TabSettle>,

// PublicView gains: names (never tab ids) of players who settled a tab in
// the immediately previous round — the after-the-fact announcement (H11).
pub settled: Vec<String>,
```

- [ ] **Step 1: dealing**

`LastCallState::new`: after seating, deal every player
`tab_for(rng_seed, seat, 0).id` into `tabs` (update the constructor doc:
tabs are no longer empty at seating — DDv2 §2.6). `add_player`: the newly
seated player gets `tab_for(self.rng_seed, seat, 0).id` (inside the
already-bumping success path; idempotent replays and full-table `None` deal
nothing). In `advance_beat`'s Draw→Deal arm, after Task 2's reveal: every
**Alive** player with empty `tabs` is dealt
`tab_for(seed, seat, nth).id` where `nth` = their `tab_ledger` entry count —
the §5 replacement rule, which also backfills a skewed old blob's `[]`
(H7).

- [ ] **Step 2: detection, settlement, void**

At `resolve()` entry, beside Task 2's `spent` snapshot:
`let round_plays = self.plays.clone();` — both captured before step 1 drains
anything, so detection is immune to where the resolution program empties
`plays` (H10). New step 5.5, after the soft cap, before the outcome check:
for each **Alive** player holding a tab, look up
`tab_def(tabs.last())` (unknown id → skip, fail-soft), evaluate
`tab_met(&def.check, seat, &round_plays, player, spent[seat])`; on success —

- pay the reward: `Hp(n)` → `hp += n` (no ceiling, TBD-3); `Pulls(n)` →
  refill the player's **most-depleted** vessel (lowest `pulls_left`, tie →
  lowest index), capped at `pulls_max` (H9);
- push `TabSettle { seat, tab, round: self.round }` onto `tab_ledger`;
- remove the id from `tabs` (the next Deal replaces it).

Rewards land after damage and the soft cap and before the outcome check, so
a Cliffhanger rescue is real but a settlement can never resurrect anyone
(elimination already happened in step 1/2, and detection skips the dead).

In the elimination path (inside `apply_damage`'s eliminate branch, beside
"hand to discards, effects removed"): `p.tabs.clear();` — ghosts hold no
objectives; the void is permanent because refills are Alive-only (H10).

- [ ] **Step 3: the announcement projection**

> **ERRATUM (2026-08-12, seam fix after Task 3).** The filter below,
> `t.round + 1 == self.round`, is correct for the common case but incomplete:
> it goes permanently unreachable for a tab settled in the round that ends
> the game. `resolve()`'s Step 5.5 (Task 3's Step 2 above) stamps a
> `TabSettle` with `self.round` — the round it settled in — before Step
> 7/8 run. For a non-terminal `resolve()`, Step 8's rollover bumps
> `self.round` past that stamp within the very same call, so by the time
> anyone reads `public_view()` the `+1` arm is already true; no code change
> was needed there, and no re-stamp step exists for tabs (unlike G5's
> `PactBreak`, see that plan's own erratum). But a `resolve()` that ends the
> game (D16) returns at Step 7, before Step 8 ever runs — `self.round` never
> leaves the round the tab was stamped with, so `+1` can never fire, and a
> final-round settle was silently never announced. Found reviewing Task 3's
> own "Recorded, not fixed" note, same bug class as G5, different fix shape:
> `tab_ledger` is durable history (Plan J's LOG reads it) and its `round`
> field means "the round it settled in" — full stop — so unlike `PactBreak`
> it does NOT get re-stamped to "the round it's visible in." Instead the
> filter grew a second arm: once the game is over, an entry stamped with
> exactly the frozen round is rendered too (only one round is ever "current"
> at a time, so this can't misfire on some earlier round's entry). Shipped:

```rust
// H11: names only, previous round only — "announced after the fact, never
// before", and never what it was. The tab id must not enter this projection.
// H erratum: a settle in the round the game ends on has no "round after" to
// wait for — render it on the frozen tableau too (see comment above).
settled: {
    let ended = self.outcome().is_some();
    self.tab_ledger
        .iter()
        .filter(|t| t.round + 1 == self.round || (ended && t.round == self.round))
        .filter_map(|t| self.players.get(t.seat).map(|p| p.name.clone()))
        .collect()
},
```

Add `settled: Vec::new(),` to `ring_fixture()`'s literal.

- [ ] **Step 4: Tests**

```rust
#[test]
fn test_tabs_are_dealt_at_seating() { // H7 — seed 42 pins from Task 1
    let st = seated();
    assert_eq!(st.players[0].tabs, vec!["lie-low".to_string()]);
    assert_eq!(st.players[1].tabs, vec!["high-roller".to_string()]);
    assert_eq!(st.players[2].tabs, vec!["peacemaker".to_string()]);
    let mut st = st;
    st.add_player(9, "dan");
    assert_eq!(st.players[3].tabs, vec!["bottoms-up".to_string()]);
}

#[test]
fn test_a_settled_tab_pays_and_is_replaced_at_the_deal() { // H10, H7
    let mut st = at_lock(); // alice holds lie-low (seed 42, seat 0)
    st.lock_in(1).unwrap(); // locking nothing is legal — and is the tab
    st.advance_beat().unwrap();
    st.advance_beat().unwrap();
    st.resolve().unwrap();
    assert_eq!(st.players[0].hp, 17); // +2 HP, paid at resolve
    assert!(st.players[0].tabs.is_empty()); // settled, awaiting the Deal
    assert_eq!(st.tab_ledger, vec![TabSettle {
        seat: 0, tab: "lie-low".into(), round: 1,
    }]);
    assert_eq!(st.players[1].tabs, vec!["high-roller".to_string()]); // unmet: kept
    st.advance_beat().unwrap(); // round 2's Deal
    assert_eq!(st.players[0].tabs, vec!["showboat".to_string()]); // nth 1 (Task 1 pin)
}

#[test]
fn test_showboat_pays_pulls_into_the_emptiest_vessel() { // H9's Pulls reward
    let mut st = at_lock();
    st.players[0].tabs = vec!["showboat".into()];
    for id in ["beer-01", "beer-02", "beer-03"] { // 1 + 2 + 1 = 4 pulls
        st.arm(1, id).unwrap();
    }
    st.set_target(1, "beer-01", Some(1)).unwrap();
    st.set_target(1, "beer-02", Some(1)).unwrap(); // beer-03 targets self
    st.lock_in(1).unwrap();
    st.advance_beat().unwrap(); // charges 4: vessel 8 -> 4
    st.advance_beat().unwrap();
    st.resolve().unwrap();
    assert_eq!(st.players[0].vessels[0].pulls_left, 6); // 4 + 2 refund
    assert_eq!(st.tab_ledger.len(), 1);
}

#[test]
fn test_cliffhanger_and_deep_pockets_read_end_of_round_state() {
    let mut st = at_lock();
    st.players[1].tabs = vec!["cliffhanger".into()];
    st.players[1].hp = 5;
    st.players[2].tabs = vec!["deep-pockets".into()];
    st.players[2].hand = std::iter::repeat_n(crate::lc_cards::deck_cards(Deck::Soft), 1)
        .flatten()
        .collect(); // 8 distinct — exactly the threshold
    st.advance_beat().unwrap();
    st.advance_beat().unwrap();
    st.resolve().unwrap();
    assert_eq!(st.players[1].hp, 8); // 5 + 3
    assert_eq!(st.tab_ledger.len(), 2);
}

#[test]
fn test_peacemaker_requires_a_harmless_play() {
    let mut st = at_lock();
    st.players[2].tabs = vec!["peacemaker".into()];
    st.arm(3, "soft-06").unwrap(); // Damage 2 — hostile
    st.set_target(3, "soft-06", Some(0)).unwrap();
    st.lock_in(3).unwrap();
    st.advance_beat().unwrap();
    st.advance_beat().unwrap();
    st.resolve().unwrap();
    assert!(st.tab_ledger.is_empty()); // hostile play: not met
    // Round 2: a heal alone settles it.
    st.players[2].tabs = vec!["peacemaker".into()]; // re-pin for isolation
    st.beat = Beat::Lock;
    st.arm(3, "soft-01").unwrap();
    st.set_target(3, "soft-01", Some(0)).unwrap();
    st.lock_in(3).unwrap();
    st.advance_beat().unwrap();
    st.advance_beat().unwrap();
    st.resolve().unwrap();
    assert_eq!(st.tab_ledger.len(), 1);
    assert_eq!(st.tab_ledger[0].tab, "peacemaker");
}

#[test]
fn test_elimination_voids_the_tab() { // H10
    let mut st = at_lock();
    st.players[1].hp = 2;
    st.arm(1, "beer-02").unwrap(); // Damage 4 kills bob
    st.set_target(1, "beer-02", Some(1)).unwrap();
    st.lock_in(1).unwrap();
    st.advance_beat().unwrap();
    st.advance_beat().unwrap();
    st.resolve().unwrap();
    assert_eq!(st.players[1].status, Status::Eliminated);
    assert!(st.players[1].tabs.is_empty()); // ghosts hold no objectives
    assert!(st.tab_ledger.is_empty());      // voided, not settled
    st.advance_beat().unwrap();             // next Deal
    assert!(st.players[1].tabs.is_empty()); // refills are Alive-only
}

/// MANDATORY secrecy pin (the tab-side twin of Plan D's §3.4.1 test): tab
/// identity never enters the projection, in any beat, in any field — and
/// the announcement carries the name, never the tab.
#[test]
fn test_tabs_are_absent_from_public_view_in_every_beat() { // H8/H11
    let mut st = at_lock(); // tabs dealt: lie-low / high-roller / peacemaker
    st.tab_ledger.push(TabSettle { seat: 0, tab: "lie-low".into(), round: 1 });
    st.round = 2;
    for beat in Beat::ORDER {
        st.beat = beat;
        let json = serde_json::to_string(&st.public_view()).unwrap();
        for needle in ["lie-low", "LIE LOW", "high-roller", "HIGH ROLLER",
                       "peacemaker", "tabs"] {
            assert!(!json.contains(needle), "beat={beat:?} leaked {needle}");
        }
        assert!(json.contains("alice"), "the announcement names the player");
    }
    assert_eq!(st.public_view().settled, vec!["alice".to_string()]);
    st.round = 3; // one round later the announcement expires
    assert!(st.public_view().settled.is_empty());
}
```

Also sweep for existing tests the seating deal disturbs (any assertion that
`tabs` is empty, or a serde fixture rebuilt by hand) — the constructor doc
comment is the one known edit; anything else surfaced by `verify.sh` is the
same mechanical kind, recorded in the report.

- [ ] **Step 5: Commit**

```bash
git add drinkinggame/src/last_call.rs drinkinggame/src/lc_render.rs
git commit -m "feat(lastcall): tabs in the engine — deterministic deal, resolve-time detection, ledger, ghost void"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

### Task 4: The public surface — the banner strip

**Class:** C (a change to what every subscriber's broadcast frame contains —
the "who is subscribed and what are they looking at" judgment plan-economics
reserves for a reviewer; the strip must carry the event and the settlement
names to every phone and the spectator screen while carrying tab identity to
none of them)

**Why this class:** the builder edit is testable, but the cross-surface
claim — one strip, two alternating occupants, zero private leakage, riding
an SSE frame all three surfaces swap — is a broadcast-content invariant.

**Files:**
- Modify: `drinkinggame/src/lc_render.rs` (`lc_banner`; tests)
- Modify: `drinkinggame/assets/lastcall.css` (strip rules)
- Test: `drinkinggame/src/lc_render.rs`, `drinkinggame/tests/http.rs`

**Interfaces:**
- Consumes: Task 2's `PublicView.event`, Task 3's `PublicView.settled`;
  `lc_events::event_def` (title/text by id, fail-soft); E10's banner/timer
  structure; `html_escape`.
- Produces: the `.lc-event` strip inside `#lc-banner` — one element, at most
  one occupant: the event from Deal onward, the settlement names at Draw,
  nothing under `outcome` (H6/H11).

- [ ] **Step 1: the strip**

In `lc_banner`, between the meta span and E10's timer child, insert exactly
one of (first match wins — the precedence IS the "never two" rendering):

1. `view.outcome.is_some()` → nothing (the game-over banner owns the moment,
   E13).
2. `view.event` is `Some(id)` and `event_def(id)` resolves →

```html
<div class="lc-event" data-event="{id}"><span class="lc-event-name">{TITLE}</span><span class="lc-event-text">{text}</span></div>
```

3. `view.beat == Beat::Draw` and `view.settled` is non-empty → one line per
   name (names through `html_escape`, uppercased):

```html
<div class="lc-event" data-settled><span class="lc-event-name">{NAME} SETTLED A TAB</span></div>
```

4. otherwise → nothing.

An id `event_def` does not know renders nothing (H3's fail-soft, applied at
the display layer too). No behaviour attributes, no hex — the builder joins
the `no_hex` sweep and `test_no_builder_emits_behaviour`'s output list.

- [ ] **Step 2: CSS**, in the shell section of `lastcall.css` (existing
tokens only — the strip is deliberately hue-neutral so it reads under every
beat colour):

```css
/* Plan H: the banner's event strip — one occupant at a time (decision H6):
   the round's event from Deal on, settlement announcements at Draw. */
.lc-event { display: flex; align-items: baseline; gap: 8px; padding: 3px 0 1px;
            border-top: 1px solid var(--lc-hairline); min-width: 0; }
.lc-event-name { font-family: var(--font-ui); font-weight: 800; font-size: 11px;
                 letter-spacing: .12em; white-space: nowrap; color: var(--lc-text); }
.lc-event-text { font-family: var(--font-ui); font-size: 11px; color: var(--lc-faint);
                 overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
```

(If the tree's token names differ — `--lc-hairline`/`--lc-faint`/`--lc-text`
are the Plan A names — use the ones `lastcall.css` actually defines; no new
colour values either way.)

- [ ] **Step 3: tests.** In `lc_render.rs` (fixtures via `ring_fixture`):

```rust
#[test]
fn test_the_banner_strip_shows_the_event_or_the_settlements_never_both() {
    // event Some("happy-hour"), settled ["alice"], beat Lock:
    //   contains data-event="happy-hour", "HAPPY HOUR", the rules text;
    //   does NOT contain "SETTLED A TAB" (the event outranks it — though
    //   H2's lifecycle means the two never truly coexist, the renderer
    //   still picks one).
    // event None, settled ["alice", "bob"], beat Draw:
    //   two data-settled lines, "ALICE SETTLED A TAB", "BOB SETTLED A TAB",
    //   no data-event.
    // event None, settled ["alice"], beat Lock: no .lc-event at all
    //   (announcements are a Draw-beat thing, H11).
    // event Some("happy-hour"), outcome Some(Winner(0)): no .lc-event
    //   (game over owns the banner, H6).
    // event Some("closing-time") (unknown id): no .lc-event (fail-soft).
    // Every variant: exactly `html.matches("class=\"lc-event\"").count() <= 1`
    //   except the two-settlement case, and no_hex on all of them.
}
```

In `http.rs` (rig: the Plan E rig with alice + bob, both vessels registered,
still at round 1 Draw — the seed is whatever `lc_start_handler` rolled, so
assertions are shape-based, not id-based):

```rust
#[tokio::test]
async fn test_the_deal_reveals_exactly_one_event_on_the_wire() {
    // Open SSE, drain the snapshot. POST /lastcall/begin -> 204 (E's chain:
    // Draw -> Deal, where the reveal fires, -> Diplomacy).
    // read_sse_until(&mut body, "data-event=") — the lcpublic frame's banner
    // template carries the strip. In that frame:
    //   - exactly ONE `data-event=` occurrence (never two at once, on the
    //     wire where it matters);
    //   - no "SETTLED A TAB";
    //   - no tab id or title ("lie-low", "LIE LOW", "high-roller",
    //     "HIGH ROLLER") — the tab deal happened at seating and the Deal
    //     edge, and none of it may ride a broadcast frame.
    // GET /room/{code}/screen: the page's server-rendered banner carries the
    // same single data-event and the same absences.
}

#[tokio::test]
async fn test_a_settlement_announces_the_name_not_the_tab() {
    // Rig at round 2 Beat::Draw with tab_ledger = [TabSettle { seat: 0,
    // tab: "lie-low", round: 1 }] via set_game_state (alice's real seat).
    // GET /room/{code}/lastcall (the phone shell): banner contains
    // "ALICE SETTLED A TAB" (rig names uppercase-safe), and neither
    // "lie-low" nor "LIE LOW" anywhere in the page.
    // GET /room/{code}/screen: same pair of assertions.
}
```

- [ ] **Step 4: Commit**

```bash
git add drinkinggame/src/lc_render.rs drinkinggame/assets/lastcall.css drinkinggame/tests/http.rs
git commit -m "feat(lastcall): the banner strip — the round's event on every surface, settlements by name only"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

### Task 5: The private surface — the tab card in the hand pane

**Class:** C (a new private surface: the one place tab identity may render,
gated on the viewer's session — the cross-viewer secrecy claim spans the
route layer and the broadcast layer and is exactly what plan-economics sends
to a reviewer)

**Why this class:** the http tests encode the leak checks, but "the hand
fragment is the ONLY surface that renders a tab, and only the viewer's own"
is a property of every surface at once, including ones this task does not
edit.

**Files:**
- Modify: `drinkinggame/src/lc_render.rs` (`lc_tab_panel`; tests)
- Modify: `drinkinggame/src/lc_routes.rs` (`hand_pane_html` appends the
  panel — the second H13 named edit)
- Modify: `drinkinggame/assets/lastcall.css` (tab-card rules)
- Test: `drinkinggame/tests/http.rs`

**Interfaces:**
- Consumes: Task 1's `TabDef`/`tab_def`/`TabReward`; Plan E's
  `hand_pane_html` assembly (`{lc_hand_pane}{targets_section}{template}`);
  `html_escape`.
- Produces (exact):

```rust
// lc_render.rs — private-side builder (the ActionBarView precedent):
// rendered only into the per-viewer hand fragment, never broadcast.
pub fn lc_tab_panel(tab: Option<&crate::lc_tabs::TabDef>) -> String;
```

- [ ] **Step 1: the builder**

`lc_tab_panel(Some(def))` (reward line: `Hp(n)` → `PAYS +{n} HP`,
`Pulls(n)` → `PAYS +{n} PULLS`):

```html
<section class="lc-tabcard" data-tab="{id}"><h2>YOUR TAB</h2><span class="lc-tabcard-name">{TITLE}</span><p class="lc-tabcard-text">{text}</p><span class="lc-tabcard-pay">PAYS +{n} {UNIT}</span></section>
```

`lc_tab_panel(None)`:

```html
<section class="lc-tabcard" data-tab-settled><h2>YOUR TAB</h2><p class="lc-tabcard-text">TAB SETTLED — a new one comes at the deal.</p></section>
```

Static catalog strings need no escaping, but pass title/text through
`html_escape` anyway — the builder must not rely on the catalog staying
tame. Joins the `no_hex` and no-behaviour sweeps.

- [ ] **Step 2: the route-side append**

In `hand_pane_html` (Plan E's assembly), insert the panel between the targets
section and the actions template:

```rust
let tab = ctx_player_seat // the viewer's seat, already looked up
    .and_then(|s| st.players[s].tabs.last())
    .and_then(|id| crate::lc_tabs::tab_def(id));
// unseated viewer: no panel at all (spectating members hold no tab)
let tab_panel = match seat {
    Some(s) if st.players[s].status == Status::Alive => {
        lc_render::lc_tab_panel(tab)
    }
    _ => String::new(),
};
```

— exact wiring follows whatever variable names E's executed `hand_pane_html`
uses; the fragment's root `#lc-hand` still leads, so the seq gate and
stale-drop are untouched and the panel rides the same private fetch (H13).
An Eliminated viewer gets no panel (their tabs were voided; the E7 hint
already tells them they're out).

- [ ] **Step 3: CSS**, in the shell section:

```css
/* Plan H: the private tab card (decision H13) — hand pane only. */
.lc-tabcard { margin: 10px 14px; padding: 10px 12px; border-radius: 10px;
              background: var(--lc-raised); border: 1px solid var(--lc-hairline); }
.lc-tabcard h2 { font-size: 10px; letter-spacing: .16em; color: var(--lc-faint); }
.lc-tabcard-name { font-family: var(--font-display); font-weight: 900;
                   font-size: 18px; letter-spacing: -.01em; color: var(--lc-text); }
.lc-tabcard-text { font-size: 13px; color: var(--lc-faint); margin: 4px 0 6px; }
.lc-tabcard-pay { font-family: var(--font-mono); font-size: 11px;
                  letter-spacing: .1em; color: var(--lc-text); }
```

(Same token-name caveat as Task 4.)

- [ ] **Step 4: tests.** In `lc_render.rs`:

```rust
#[test]
fn test_the_tab_card_states_its_deal() {
    // Some(lie-low): data-tab="lie-low", "YOUR TAB", "LIE LOW", the text,
    //   "PAYS +2 HP". Some(showboat): "PAYS +2 PULLS".
    // None: data-tab-settled, "TAB SETTLED", and no data-tab= attribute.
    // no_hex on all three.
}
```

In `http.rs` (rig: the Plan E rig; then `set_game_state` pins
`players[alice].tabs = ["lie-low"]`, `players[bob].tabs = ["showboat"]` for
determinism regardless of the room's rolled seed):

```rust
#[tokio::test]
async fn test_a_tab_is_visible_to_its_holder_alone() { // the plan's secrecy gate
    // GET hand as alice: contains `data-tab="lie-low"` and "LIE LOW";
    //   does NOT contain "showboat" or "SHOWBOAT".
    // GET hand as bob: contains "SHOWBOAT"; does NOT contain "lie-low" or
    //   "LIE LOW".
    // Open SSE, POST /lastcall/handicap (any full publish), read_sse_until
    //   "event: lcpublic": the broadcast frame contains NEITHER tab id nor
    //   title — same-transport proof that the hand fetch is the only road
    //   a tab travels.
    // GET /room/{code}/lastcall/table as alice: no lc-tabcard, no tab id —
    //   the TABLE fragment stays tab-free (H13).
}

#[tokio::test]
async fn test_a_settled_tab_shows_the_placeholder_card() {
    // set_game_state with alice's tabs = [] (settled, pre-Deal).
    // GET hand as alice: contains data-tab-settled and "TAB SETTLED";
    // no data-tab= attribute.
}
```

- [ ] **Step 5: Commit**

```bash
git add drinkinggame/src/lc_render.rs drinkinggame/src/lc_routes.rs drinkinggame/assets/lastcall.css drinkinggame/tests/http.rs
git commit -m "feat(lastcall): the private tab card — hand pane only, viewer's own tab only"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

## Browser checkpoint — after Task 5, before the final review

A human, a real focused browser, `cargo run -p drinkinggame`, two profiles,
a room, START on the Last Call card, plus the big screen in a third window:

1. Register drinks and press START ROUND 1: the banner grows the event strip
   at the Deal — same event, same wording, on both phones and the big
   screen; the strip fits without breaking F.1's fixed vertical order or the
   88px screen header.
2. Each phone's HAND pane shows a YOUR TAB card — and the two phones show
   **different** tabs (deterministic deal, different seats).
3. Play a round where one player's tab settles (Lie Low is the easy one:
   lock nothing). At the next round's Draw the banner reads "{NAME} SETTLED
   A TAB" on every surface, without saying what it was; that player's HAND
   pane shows the settled placeholder, then a fresh tab after the Deal.
4. Watch one mechanical event actually bend the round — Happy Hour's DRINK
   chip halving is the visible one (arm a cost-2 card, see DRINK 1).
5. Confirm the LOG tab is still its empty pane, and the TABLE tab shows no
   tab card.

## Before the plan is done

- Tasks 1–3 are Class B (their tests are the spec, including both secrecy
  pins); Tasks 4–5 are Class C and each gets its per-task reviewer on a
  capable model. The Task 4 brief must name: the strip's one-occupant
  precedence, the frame-content leak checks, and that no publish path
  changed. The Task 5 brief must name: the hand fragment as the only
  tab-bearing surface, the unseated/eliminated gating, and §6.1's
  no-identifier route left untouched. **One whole-plan review** of the
  branch diff at the end, on the most capable model, covers Tasks 1–3.
- No `cargo sqlx prepare` (no migration; `drinkinggame` is runtime-checked).
- Interfaces line up: Task 1's `EventHook`/`TabCheck` are what Task 2/3's
  engine matches on; Task 2's `charged_pulls` feeds Task 3's `spent`
  snapshot and E's retuned DRINK chip; Task 2's `PublicView.event` and Task
  3's `.settled` are what Task 4 renders; Task 1's `TabDef` is what Task 5's
  builder takes; the ledger `TabSettle` is named as Plan J's input.
- Every spec requirement maps: §10.1 list + "never two at once" + guardrail
  → H1–H6 / Tasks 1–2–4; §10.2 list + detection + "announced after the
  fact" → H7–H11 / Tasks 1–3–5; §2.6 seating deal → Task 3; §5 beat-2
  duties (reveal + replacement) → Tasks 2–3's shared Draw→Deal hook; spec
  §9's doc-conflict rulings (no telegraphed next event; `tabs[]` over
  "quests") → H2/H8.
- STATUS updated: events and tabs shipped (two of the "hollow systems"
  closed); the cut lists (H5, H9's vendettas), the end-of-game unsettled-tab
  reveal (Plan J, reads `tab_ledger` + `tabs`), and the H14 prime-count
  caveat recorded as open notes.
- `drinkinggame` stays clippy-clean; distinct-warning count stays 17.

## Self-review (performed while writing)

- **Scope coverage vs the brief:** event list proposed as a reviewable table
  with trigger timing, op-vocabulary effects and durations ✔ (H4; every
  effect resolves through `apply_damage`/`drain_pulls`/heal — no new
  `EffectOp` was needed, so none was added, and the five DDv1 events that
  would have needed one are cut by name in H5); selection deterministic from
  `rng_seed` + round, no RNG ✔ (H1); "never two at once" binding — held by
  type, by lifecycle, and pinned on the wire ✔ (H2, Task 2/4 tests); events
  ride `lcpublic` into the banner with placement argued against F.1/F.2 ✔
  (H6); tab list proposed as a reviewable table with predicates and rewards
  ✔ (H9); detection timing decided (inside `resolve()`, snapshot-based) ✔
  (H10); `tabs` stays `Vec<String>` id+lookup for the serde-strictness
  reason the brief pointed at ✔ (H8); completion announcement decided: yes,
  name only ✔ (H11); tabs render only in the private fragment, LOG untouched
  ✔ (H13); CSS in `lastcall.css`, no JS needed so `lc_loop.js` untouched ✔;
  secrecy tests at both layers, never-two pinned, selection pinned with
  expected values ✔.
- **Placeholder scan:** no TBD/TODO; every id, title, rules text, magnitude,
  formula constant, CSS rule and pinned expectation is spelled out; prose
  only where a named pattern exists (Plan D's guard shapes, E's assembly,
  F's catalog module shape).
- **Type consistency:** consumed signatures copied from D/E/F's Produces
  blocks and flagged as such (all three unexecuted at writing time);
  `payment_plan`'s new parameter is the one D-signature change and is named;
  the two `lc_routes.rs` edits are enumerated and bounded; `ring_fixture`
  gains its two literal fields in the tasks that add the projection fields.
- **Known cross-plan risks, flagged upward:** (1) Plan G is unwritten — if
  it stores pact markers in `tabs[]`, H's fail-soft tolerates it, but the
  Task 3 executor must check the tree and report the seam. (2) The seed-42
  pins in Tasks 2–3 depend on `seated()`/`at_lock()` keeping seed 42 and the
  EVENTS/TABS array orders in Task 1 — both are named as load-bearing where
  they are defined. (3) Token names in Tasks 4–5's CSS are Plan A's; the
  executor substitutes the tree's actual names if they differ, changing no
  colour values.
