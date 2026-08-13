# Drinks: test play mode — fake players + identity switcher

**Status:** in progress — approved 2026-08-13 ("go ahead")
**Branch:** `feat/last-call-refinement`

Solo playtesting from one browser: spawn fake players into a room and hop
between identities, so every seat of a Last Call table can be driven from
one phone. Gated by the `DRINKS_TEST_MODE=1` env var so the live server
can never expose it (unset ⇒ routes 404, no UI renders).

## Items

- [ ] **1. Plumbing + routes** — `test_mode` on `GameState` (read from
      `DRINKS_TEST_MODE` at startup); `POST /room/{code}/test/spawn`
      (create a `test-N` player, join the room, seat/vessel if a Last Call
      lobby is open); `POST /room/{code}/test/act-as` (re-issue the session
      cookie as any member, 303 back to the room). Both 404 when the flag
      is off.
- [ ] **2. Switcher bar** — when the flag is on, a slim bar on the room
      page and the Last Call shell: one button per member (current
      highlighted), plus "+ FAKE". Plain form posts, no JS.
- [ ] **3. Tests + pack gate** — routes 404 with the flag off; spawn/
      act-as flow with it on; `./scripts/verify.sh`; solo browser
      walkthrough using the bar itself.

## Ledger

- 2026-08-13: pack opened (item list pre-approved with the earlier
  proposal).
