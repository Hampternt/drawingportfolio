# Last Call: mobile play flow (container)

**Status:** decisions resolved 2026-08-13 — awaiting go on the pack sequence
**Branch:** `feat/last-call-refinement` (existing stream; merges → `dev` → `master`)
**Design source:** `docs/design/lastcall-mobile/` — HANDOFF.md (spec) +
prototype.html (interactive 390×844 reference), delivered 2026-08-13 as
"Mobile Last Call game view.zip". This is the "table-screen card-play
design" the stream was left open for.

## Goal

Recreate the design-handoff phone experience: HAND becomes a pure reading
surface (wheel + inspect sheet), TABLE becomes the play surface (fanned
tray → drag/tap targeting overlay → arm flash with dotted arrows → ARMED
stack → LOCK IN), plus a Gwent-style mulligan overlay, a mode badge, and
a slide-out side-quest drawer.

**Engine fit (surveyed 2026-08-13):** the engine and routes already cover
the whole flow — `arm`/`disarm`/`target`/`lock`/`mulligan` all exist, the
armed queue is `LcPlayer.armed: Vec<ArmedCard>`, take-back = disarm with
pulls refund. This container is ~90% presentation: `lc_render.rs` builders,
`lastcall.css`, `lc_loop.js`/`lc_wheel.js`/`lc_motion.js`, plus small
route-glue. The only candidate engine change is D2 (and D1 if the user
prefers the design's mulligan semantics).

## Packs

Only the active pack gets an item manifest; the lists below are the
proposed shape, one level deep.

### Pack 1 — TABLE becomes the play surface

Observable: play a card by dragging it from a bottom tray onto a player;
armed plays sit in a stack with dotted arrows until LOCK IN.

Proposed items: fanned-mini tray (`.lc-tray`) · full-pane targeting
overlay (drop-zone rows + staged-card preview; tap and pointer-drag paths;
posts `arm`+`target`) · arm-flash + curved dotted arrows / AOE wave +
fly-to-stack (extends `lc_motion.js`) · ARMED-stack redesign (58px minis,
"cost → target" line, persistent arrows, tap-to-take-back) · retire the
`.lc-targets` native-select rows · LOCK IN label "· n QUEUED".

### Pack 2 — HAND is a reading surface + shell chrome

Observable: tapping the focused wheel card opens a full-rules inspect
sheet whose "PLAY ON THE TABLE →" jumps to a staged TABLE tab; the tab
row carries the mode badge + pull count.

Proposed items: inspect sheet (bottom sheet, expanded face, 2×2 meta
grid) with the wheel-tap rewire (D3) · mode badge (READ / PLAY / TARGET /
ARMING / MULLIGAN / LOG) + pull count + note row · focused-card
affordances (TAP TO READ hint, position counter, hint line, stage
`clip-path`) · side-quest "YOUR TAB" drawer replacing the inline card.

### Pack 3 — Mulligan overlay + felt polish

Observable: card swaps happen in a full-screen multi-select overlay; seat
chips carry hand-composition strips (if D2 approves).

Proposed items: mulligan overlay UI over the existing route (semantics
per D1) · seat-chip hand-composition strips (needs a `public_view`
change — D2) · felt/centre polish ("PLAYS n", chip flash states).

## Decisions (resolved 2026-08-13)

- **D1 — Mulligan semantics: keep the engine's.** Per-card discard/redraw,
  free in the round-1 lobby, once per round from round 2, same-deck
  replacements (`mulligan()`, `last_call.rs:1108`). The overlay UI adopts
  these semantics; copy says per-round, not once-per-game.
- **D2 — Opponent hand strips: ship them.** User wants visible hand
  composition. Engine change: `public_view` exposes per-deck card counts
  for every seated hand (identity of cards stays private; arm stays
  tick-only). Privacy tests updated to pin the new line: counts public,
  cards secret.
- **D3 — Wheel-tap rewire: confirmed.** Tap = read (inspect sheet);
  instant-arm gesture retires; playing moves to the TABLE tray and the
  sheet's PLAY button.
- **D4 — END GAME form: keep as-is,** below the new tray, still a sibling
  of `#lc-table` so table repaints leave it alone.

## Standing constraints (from the survey)

- New `.lc-*` component roots and `@keyframes` must be registered in
  `tests/http.rs` (`:224` roots list, `:263` keyframes list); reduced
  motion stays one block (`:280`).
- Builders emit `data-*` only — no `hx-*`/`onclick`/`href`/`action=`
  (`test_no_builder_emits_behaviour`); behavior lives in `lc_loop.js`'s
  delegated listeners.
- `arm` stays a tick-only broadcast (card identity never leaves the
  owner); hand fragment stays session-only.
- Render tests → `lc_render.rs`, engine tests → `last_call.rs`, wiring →
  `tests/http.rs` (rigs at `:5866` / `:7606`). Workspace count 751 moves —
  re-measure at each pack gate.
- Motion: 130–280ms, `cubic-bezier(.2,.8,.3,1)`, honor
  `prefers-reduced-motion` (existing block).

## Ledger

- 2026-08-13: design zip delivered on `dev`; extracted to
  `docs/design/lastcall-mobile/`; engine/UI survey done; container
  drafted. Awaiting D1–D4 + go.
- 2026-08-13: D1 engine-semantics, D2 ship per-deck strips (public_view
  change), D3 confirmed, D4 keep. Awaiting go on the pack sequence.
