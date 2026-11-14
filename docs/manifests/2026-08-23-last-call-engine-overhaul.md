# Last Call engine overhaul — real decks, five effect systems, the round report

**Status:** merged via Hampternt/drawingportfolio#13 (2026-10-31) — **the
owner's walkthrough is still to do**: run *Test it yourself* below against
the deployed build or a local one.

**Stream:** `claude/last-call-game-overhaul-2laeqa`, from `master`,
2026-08-23. Built 2026-08-23 → 2026-08-30, idle until 2026-10-31, when it
was brought up to date with `master` and finished for merge. This file was
written at that handover; the build itself ran without one, and PR #13's
description is the long-form record.

## Goal

Make Last Call's engine trustworthy enough to build new kinds of cards on:

- Decks that are real piles of cards, so a rare card is actually rare and a
  game replays identically from its seed.
- Card effects beyond "a number on HP": cards that reveal information, make
  people drink or discard, swap cards between hands, and pin a social rule
  on a player.
- After every round, a plain record of what hit whom with which card — the
  thing a player watching their HP drop could not previously find out.

## Decisions

- **Real piles over a counter.** `deck_counts` was replaced by `Shoe` draw
  and discard piles in `lc_deck`; not chosen: keeping the counter and
  sampling without replacement in the routes.
- **The engine owns a seeded RNG** (`lc_rng`, stored in the blob); the
  room-creation seed is the one entropy boundary. Not chosen: route-level
  `thread_rng()`.
- **Phase is derived, never stored** (`lc_phase`), read off
  `PublicSeat::phase` by every client.
- **New effects are catalog-side, resolved by id** — one
  `eff: Option<CardEffect>` enum (`Reveal`/`Pour`/`Swap`/`Rule`), collapsed
  from per-system `Option`s at the sixth system as CLAUDE.md instructed.
- **The round report is a record beside the log, not a log filter**: it
  carries what the log omits (a fully shielded hit logs nothing).
- **Only seats a blow landed on gate the round.** A round that lands nothing
  does not park; a tap from anyone else is accepted and changes nothing.
- **Every new card ships at `copies: 0`.** Reveal, pour, swap and rule cards
  have no picker UI yet, and pours and swaps park the round — so no live
  game can draw one. `test/grant` puts them in a hand for testing.
- **Merged before the owner's walkthrough** (owner's call, 2026-10-31): the
  test is kept as a to-do rather than a gate. Not chosen: holding the merge
  until it ran. What stood in for it is the full gate and the browser
  smoke under *Built*.
- **Old saves are rebuilt from their own seed**: hands, HP, effects, pacts
  and log survive; draw order and discard piles reset.

## Guidelines

- Read first: `CLAUDE.md` §Last Call's engine (the five rules not to
  break), `drinkinggame/ENGINE.md` (status, what's missing),
  `drinkinggame/RULES.md` (players), `drinkinggame/CARDS-DRAFT.md` (the
  card waves), PR #13's description.
- Every card movement through `lc_deck`; every HP/pull change through
  `damage`/`heal`/`shield`/`drain`; every settle path asks `parked()`.
- Anything that parks a round needs a settle route before its cards leave
  `copies: 0`.

## Done when

- A Last Call game plays end to end on the new engine, and the big screen
  and phones render as they did on `master`.
- After a round in which someone was hit, a **THE ROUND** strip lists each
  blow — attacker, card, target, amount — and the table waits until each
  player who was hit taps **GOT IT**.
- A round in which nothing lands rolls straight over with no strip.
- A blocked hit shows in the strip as **BLOCKED**, even though the log
  stays silent.
- A seat carrying a standing rule shows it as a badge on its plaque.

## Test it yourself

Load the test data: none needed — test play mode makes fake players.
Run on a throwaway database so nothing touches your dev rooms:

```bash
DRINKS_TEST_MODE=1 DRINKS_DATABASE_URL='sqlite:/tmp/lc-test.db?mode=rwc' cargo run -p drinkinggame
```

1. Open http://localhost:3001, log in with any name and a 4-digit PIN,
   create a room. Tap **+ FAKE** twice → three seats in the TEST bar.
2. In a second tab open `/room/<CODE>/screen` (the big screen). → Seat
   plaques sit on the felt, each with a coloured stripe along its top
   (the drink's colour once registered).
3. Start Last Call. For each seat (switch with the TEST chips), pick a drink
   and **REGISTER A DRINK**, then **START ROUND 1**. → Everyone is dealt a
   hand; the big screen's DECKS LEFT shows the registered drinks' decks with
   what is left after the deal.
4. Play one round with nobody playing a card: ready every seat through to
   resolve. → No report strip; the round rolls straight over.
5. Next round, have one seat play a damage card on another, lock in, and
   ready everyone through Reveal. → A **THE ROUND** strip lists
   `attacker → card → target  -N`. The table does not advance.
6. As a seat that was *not* hit, tap **GOT IT**. → Nothing changes. As the
   seat that was hit, tap it. → The round rolls over.
7. Shield a seat, then hit it for no more than the shield. → The strip reads
   **BLOCKED**; the LOG tab shows no hit line for it.
8. Standing rule: as any seat, `POST /room/<CODE>/lastcall/test/grant` with
   `card_id=wine-12` (from the browser console:
   `fetch(location.pathname + '/lastcall/test/grant', {method:'POST', body:new URLSearchParams({card_id:'wine-12'})})`),
   then, once that seat has the 2 pulls it costs, play **Hat of
   Compliments** on any seat (itself included) and resolve. → That seat's plaque
   carries the rule's text as a badge for two rounds, and its deck stripe
   keeps its colour.

## Built

**What changed** — 18 commits on the branch, then the merge and one fix on
2026-10-31. Full narrative in PR #13; in short:

- Engine: `lc_deck` (real piles, cards conserved), `lc_rng` (SplitMix64,
  stored), `lc_phase` (derived `Phase`/`SeatPhase`), `lc_triggers`
  (card-triggered events — recorded and projected, deliberately inert: no
  route acknowledges them), `lc_report` (the round report and its gate). All
  HP/pull changes routed through the four primitives.
- Effects: the `"other"` target class; `Reveal`, `Pour`, `Swap` and `Rule`
  under one `CardEffect` enum; ten new cards, all `copies: 0`.
- Surfaces: the round-report strip with its **GOT IT** button
  (`/lastcall/resolution-ack`), and the standing-rule badge on plaques.
- Bugs fixed along the way: short opening hands, unreachable
  `OnPlay`/`OnDiscard`, a spectator-screen discard-count misalignment, two
  missed drink counts, a panic when a pour or trade parked the round, and
  seven tests that covered nothing (each proved by mutation).

**On 2026-10-31, done differently from the original branch:**

- Merged `master` (116 commits — Backroom, the seat ring, the felt table).
  Conflicts were docs only: one block each in `CLAUDE.md` and
  `docs/design.md`, both additive. Master's only Last Call code change
  (`room_members` → `seated_members` in `lc_routes`) merged cleanly.
- `docs/design.md`'s Last Call section corrected on three stale claims: it
  now names five modules (adds `lc_report`), says the trigger queue is
  inert rather than that it holds the table, and says the save-rebuild guard
  is an early-out with `LcTable::open`'s `is_open` as the real protection
  (CLAUDE.md had been corrected, design.md had not).
- **Regression found on the smoke load and fixed:** the standing-rule badge
  reused the class `.lc-rule`, which is the plaque's deck-colour stripe — so
  every seat's stripe on every surface rendered as a grey pill. Renamed to
  `.lc-standing-rule(s)`; the plaque test now asserts only the stripe
  carries `.lc-rule`, and fails on the old markup.
- **Second regression found on the smoke load and fixed:** on the big
  screen the round report rendered as the phone's full receipt — four rows
  and a GOT IT button — inside `.lc-screen-head`, a fixed 88px box, so it
  spilled over the felt; and nobody can press a button on a cookie-less TV.
  Screen-scoped CSS now collapses it to one row: **THE ROUND · WAITING ON
  …**. The blow list stays on the phones. (Decision taken here, not by the
  owner: the TV shows who the room is waiting on, not the blows. Showing
  the blows on the TV would need a redesign of the screen header.)
- Cleared five clippy warnings in `drinkinggame` (four from this branch, one
  from master's `tests/http.rs`) — the crate is back to zero.

**Smoke load** (headless Chromium, three real players in separate
sessions, test mode, throwaway DB): played round 1 through the routes,
with one seat hitting another and playing Hat of Compliments on them.
Checked on the phone (412px) and the big screen (1280×720):
- the THE ROUND strip lists every blow, the hat as `2 ROUNDS`, and a
  random event's hit
- clicking **GOT IT** in the browser posts `resolution-ack` (204), and once
  every seat that was hit had tapped, the round rolled over and the strip
  went away
- the rule badge sits on the target's plaque, and deck stripes keep their
  colours.

Not this branch's (both also happen on `master`, so left alone):
- console: a 404 for `/favicon.ico`, which the standalone server does not
  serve
- an intermittent `htmx is not defined` from `room.html`'s `swapPanel`,
  which can run before the deferred `htmx.min.js`. That line dates from
  2026-07-31, and this branch does not touch `room.html`
- seat plaques clip at the big screen's left pane edge at 1280×720.

**Full gate** — `SQLX_OFFLINE=true ./scripts/verify.sh`, after the last code
change (only docs changed after it), exit 0:

```
test result: ok. 367 passed; 0 failed; 0 ignored   (drawingportfolio)
test result: ok. 8 passed; 0 failed; 0 ignored     (tests/static_assets.rs)
test result: ok. 709 passed; 0 failed; 0 ignored   (drinkinggame lib)
test result: ok. 255 passed; 0 failed; 0 ignored   (drinkinggame tests/http.rs)
ok   docs/design/sorting-live/src/model.test.js — passed 265/265 checks
ok   docs/design/sorting-live/src/board.test.js — passed 193/193 checks
VERIFY OK — fmt, clippy, tests, JS syntax, board suites all clean.
```

1339 workspace tests (1008 on the branch before the merge, 1185 on
`master`). Clippy: no `drinkinggame` warnings; `drawingportfolio` unchanged.
