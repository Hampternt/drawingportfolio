# Last Call: ready-advance replaces the beat clock

**Status:** done — pack gate green, browser-verified 2026-08-13
**Branch:** `feat/last-call-refinement`
**Decision (user, 2026-08-13):** remove the beat timer entirely — no clock,
no backstop. Beats wait indefinitely; Draw/Diplomacy/Reveal advance when
every alive seat taps READY, Lock keeps its existing all-locked advance.
Deal/Resolve stay auto. One AFK player stalls the table — accepted for a
living-room game. (This is the design README's original intent — "the clock
is a backstop, not a pace-setter" — minus the backstop.)

## Items

- [x] **1. Engine** — `LcPlayer.ready` (`#[serde(default)]`), `set_ready()`
      (Draw/Diplomacy/Reveal only, round-1 Draw lobby refused, idempotent),
      `all_ready()`, flag cleared on every beat edge (`advance_beat`) and at
      rollover (`resolve`), `PublicSeat.ready` projection. (f6dba14)
- [x] **2. Routes** — `POST /room/{code}/lastcall/ready` with the
      lock-handler's early-advance shape; deadlines never armed
      (`lc_advance_chain` clears instead of re-arming); delete
      `arm_beat_clock`, `extend_response_window`, `REACT_GRACE_SECS`,
      `duration_secs` + the four `*_SECS` consts; keep `lc_tick_room` as the
      migration sweep for in-flight blobs holding a stale deadline. (5aeede4)
- [x] **3. UI** — READY button in the action bar (Draw ≥ r2, Diplomacy,
      Reveal), ready ticks on seat chips + minitable, live countdown removed
      (`beat_timer_live`, lc_loop.js timer block; preview's static
      `beat_timer` stays). (443c963)
- [x] **4. Tests + pack gate** — engine/route tests, timer tests rewritten
      to assert never-armed, `./scripts/verify.sh` green, clippy at the
      documented 21 (drinkinggame clean), workspace at 746 tests
      (CLAUDE.md count updated).

## Ledger

- 2026-08-13: pack opened; design fork resolved by user ("no clock at all").
- 2026-08-13: items 1–3 landed (engine → routes → UI), one commit each.
- 2026-08-13: pack gate green (`verify.sh`; fmt fix folded into the docs
  commit). Browser walkthrough on :3001 with a curl-driven second player:
  lobby → begin → Diplomacy (READY + no countdown) → all-ready → Lock →
  all-locked → Reveal (READY) → all-ready → round 2 Draw; then held >40s
  at Draw to confirm nothing auto-advances.

<details>
<summary>Deviations & notes</summary>

- `lc_tick_room` and `beat_deadline_ms` survive as a migration sweep: a
  room persisted mid-countdown by the previous binary gets one last
  advance, then the field is `None` forever. Safe to delete both once no
  pre-removal blob can exist.
- Reveal's response window is now open-ended (decision I3's grace
  extension deleted with the clock) — a response can't be "almost too
  late"; the window closes when the table taps.
- Browser verification needed a second session driven over curl — one
  browser profile = one name+PIN session. That friction is the motivation
  for the proposed test-play-mode pack.

</details>
