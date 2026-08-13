# Last Call: ready-advance replaces the beat clock

**Status:** in progress
**Branch:** `feat/last-call-refinement`
**Decision (user, 2026-08-13):** remove the beat timer entirely — no clock,
no backstop. Beats wait indefinitely; Draw/Diplomacy/Reveal advance when
every alive seat taps READY, Lock keeps its existing all-locked advance.
Deal/Resolve stay auto. One AFK player stalls the table — accepted for a
living-room game. (This is the design README's original intent — "the clock
is a backstop, not a pace-setter" — minus the backstop.)

## Items

- [ ] **1. Engine** — `LcPlayer.ready` (`#[serde(default)]`), `set_ready()`
      (Draw/Diplomacy/Reveal only, round-1 Draw lobby refused, idempotent),
      `all_ready()`, flag cleared on every beat edge (`advance_beat`) and at
      rollover (`resolve`), `PublicSeat.ready` projection.
- [ ] **2. Routes** — `POST /room/{code}/lastcall/ready` with the
      lock-handler's early-advance shape; deadlines never armed
      (`lc_advance_chain` clears instead of re-arming); delete
      `arm_beat_clock`, `extend_response_window`, `REACT_GRACE_SECS`,
      `duration_secs` + the four `*_SECS` consts; keep `lc_tick_room` as the
      migration sweep for in-flight blobs holding a stale deadline.
- [ ] **3. UI** — READY button in the action bar (Draw ≥ r2, Diplomacy,
      Reveal), ready ticks on seat chips + minitable, live countdown removed
      (`beat_timer_live`, lc_loop.js timer block; preview's static
      `beat_timer` stays).
- [ ] **4. Tests + pack gate** — engine/route tests for the above, timer
      tests rewritten to assert never-armed, `./scripts/verify.sh`, clippy
      count still 21 (drinkinggame stays clean).

## Ledger

- 2026-08-13: pack opened; design fork resolved by user ("no clock at all").
