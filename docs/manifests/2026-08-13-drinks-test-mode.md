# Drinks: test play mode — fake players + identity switcher

**Status:** done — pack gate green, browser-verified solo 2026-08-13
**Branch:** `feat/last-call-refinement`

Solo playtesting from one browser: spawn fake players into a room and hop
between identities, so every seat of a Last Call table can be driven from
one phone. Gated by the `DRINKS_TEST_MODE=1` env var so the live server
can never expose it (unset ⇒ routes 404, no UI renders).

## Items

- [x] **1. Plumbing + routes** — `test_mode` on `GameState` (env at
      startup; `router_with_pool_flagged` is the test seam since env vars
      race across test threads); `POST /room/{code}/test/spawn` (create/
      re-login the next free `test-N`, PIN 0000, join the room); `POST
      /room/{code}/test/act-as` (re-issue the session cookie as any member,
      303 back to the room). Both 404 when the flag is off. (c9a8ad3)
- [x] **2. Switcher bar** — `render::test_switcher_bar` on the room page
      and the Last Call shell: one act-as button per member (current
      marked + inert), plus "+ FAKE". Plain form posts, no JS; loud amber
      styling duplicated into game.css and lastcall.css (the LC shell
      loads only the latter). (cf896d6)
- [x] **3. Tests + pack gate** — routes 404 + no bar with the flag off;
      spawn/act-as/bar flow with it on; `./scripts/verify.sh` green,
      clippy at the documented 21, workspace at 754 tests (CLAUDE.md
      updated).

## Ledger

- 2026-08-13: pack opened (item list pre-approved with the earlier
  proposal).
- 2026-08-13: items 1–2 landed, one commit each; pack gate green.
- 2026-08-13: solo browser walkthrough with `DRINKS_TEST_MODE=1`: one
  browser created a room, spawned test-1/test-2 from the bar, started
  Last Call, hopped identities to register three different drinks, began
  round 1, and locked all three seats — the table flipped to Reveal with
  no second device involved.

<details>
<summary>Deviations & notes</summary>

- Spawn does NOT auto-register a vessel (the manifest draft floated it):
  registering while acting as each fake exercises the real lobby UI,
  which is the point of the mode. Late joins into a running game ride the
  existing room-page late-join seating, so spawn stays game-agnostic.
- Fakes are ordinary global players (`test-N`, PIN 0000) — they appear in
  all-time stats like anyone else. Fine for a dev database; on the live
  server the mode is off, so no fakes can exist there.
- `act-as` leaves the previous session row to expire naturally; switching
  back is just another act-as.

</details>
