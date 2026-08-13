# Last Call: mobile play flow (container)

**Status:** COMPLETE 2026-08-13 — all three packs landed on
`claude/last-call-plan-review-2bia3p` (PR #11, draft, → `dev`)
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

### Pack 1 — TABLE becomes the play surface (ACTIVE)

Observable: play a card by dragging it from a bottom tray onto a player;
armed plays sit in a stack with dotted arrows until LOCK IN.

Item manifest:

1. **Private table fragment.** `table_pane_html` takes `&LastCallState` +
   `player_id` (not `&PublicView` + `me`): the TABLE fetch is already
   per-viewer/session-gated, so it may carry the viewer's OWN tray and
   armed stack. Staging surfaces render only for a seated, alive viewer
   during Diplomacy/Lock with no outcome; the mini table itself stays
   public-projection-only. Privacy test: another player's table fetch
   never contains my card titles.
2. **Fanned-mini tray** — `lc_tray(hand)` builder (`.lc-tray`): header row
   ("YOUR HAND · n" / "DRAG A CARD ONTO A PLAYER"), overlapped 64px minis
   (cost + 2-line title, deck ink border, `data-card-id/-deck/-cost/
   -targets/-kind`), hidden scrollbars, plus a hidden per-card
   `card_face` preview stash the overlay clones from.
3. **Full-pane targeting overlay** — `lc_target_overlay(view, me)` builder
   (`.lc-tgt`): one 52px row per alive seat + one EVERYONE row, deck dot,
   YOU tag, HP; hidden at rest, JS shows the row subset matching the
   staged card's `targets` class. Tap path (stage from tray) and
   pointer-drag path (ghost mini, rect hit-testing on the real rows) both
   end here; choosing a target posts `arm` then (for "one") `target`.
   Reaction cards don't arm — client note, no post.
4. **Arm flash + arrows + wave + fly-to-stack** — `lc_motion.js` grows
   `lcArmFlash` (preview clone at felt centre with "ME → TGT" caption in
   the `#lc-flights` layer, then a 450ms fly into the stack) and
   `lcTableArrows` (curved dotted SVG paths from stack minis to their
   target chips; two staggered `lc-wave` ellipses for AOE). New keyframes
   `lc-dashflow`/`lc-wave`/`lc-pop`/`lc-cardin` registered in
   `tests/http.rs`; reduced motion skips the flash and stills the dashes
   inside the existing block.
5. **ARMED stack redesign** — `lc_table_stack(...)` builder (`.lc-stack`)
   on the felt's left edge: "ARMED n" header, 58×46 minis with a
   "cost → TARGET" line, "TAP TO EDIT" footer; tap = take-back
   (`lc:disarm` → existing disarm route); `data-locked` after lock (minis
   from `locked_plays`, no take-back, header "LOCKED n").
6. **LOCK IN label** — `ActionBarView.armed_count`; Diplomacy's primary
   reads "LOCK IN · n QUEUED" when n > 0.
7. **Retire `.lc-targets`** — `targets_section_html`, its CSS and
   `lc_loop.js`'s `change` listener go; the overlay owns targeting.
   Interim note: until Pack 2's D3 rewire, a wheel-tap still arms without
   a target — the stack mini shows "— PICK" and take-back + tray re-play
   is the edit path.

### Pack 2 — HAND is a reading surface + shell chrome (LANDED)

Observable: tapping the focused wheel card opens a full-rules inspect
sheet whose "PLAY ON THE TABLE →" jumps to a staged TABLE tab; the tab
row carries the mode badge + pull count.

Shipped items: inspect sheet (`.lc-sheet`: hidden skeleton + per-card
stash in the private hand fetch — expanded face, 2×2 meta grid, CLOSE +
PLAY row; a Reaction gets the reveal-window note, never PLAY, since the
engine refuses `arm` for reactions) · wheel-tap rewire (D3: the wheel
dispatches `lc:inspect`; `lc:arm` and the Draw-beat tap-to-swap it
carried retire — the swap returns as Pack 3's overlay) · mode badge
(READ / PLAY / TARGET / ARMING / LOG, `#lc-mode-badge`; MULLIGAN joins
in Pack 3) + pull count (`data-pulls` on `#lc-hand`, summed vessel
pulls) · focused-card affordances (TAP TO READ ::after, position counter
as a stage SIBLING — inside it the 3D depth sort paints cards over any
z-index — hint line moved below the stage, stage `clip-path`) ·
side-quest drawer (the `.lc-tabcard` root name survives on purpose;
privacy tests select on it).

### Pack 3 — Mulligan overlay + felt polish (LANDED)

Observable: card swaps happen in a full-screen multi-select overlay; seat
chips carry hand-composition strips (if D2 approves).

Shipped items: mulligan overlay (`.lc-mull`, D1 semantics — copy says
free-in-lobby / once-a-round, never once-per-game; opens from the Draw
beat's new MULLIGAN action-bar button, posts comma-joined ids to the
existing route; the old wheel-tap swap hints are gone with the gesture) ·
D2 hand strips (`PublicSeat.hand_by_deck`: per-deck counts over
hand+armed+locked — the same three terms as `hand_len`, so staging never
moves the strip; identity stays private, pinned by
`test_public_view_hand_by_deck_counts_but_never_identity`; rendered as
`.lc-mix` swatches on every seat chip) · felt polish (the viewer's own
"PLAYS n" under the centre pile; chip hit-shake/heal-flash diffed off
`data-hp` in `lcTableSync`, the phone twin of the plaque pass).

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
- 2026-08-13: go given (user: run all three packs to completion). Pack 1
  activated; item manifest drafted above.
- 2026-08-13: Pack 3 lands — the container is COMPLETE. verify.sh green
  (759 → 765: +1 engine, +3 render units, +2 http). Browser-smoked: the
  overlay swaps real cards through the engine (same-deck replacements
  observed), mix strips + PLAYS render, badge shows MULLIGAN. Fix found
  live: overlay z-tiers re-cut (wheel counter 120 < sheet 400 < mulligan
  450 — the counter pill floated over the overlay at its old 500).
  Remaining known deviations, all deliberate: no arrowhead markers on
  arrows (SVG markers can't take per-deck ink portably), the note row
  folded into the action-bar hints, and the HAND pane keeps its armed
  column + cost rail (the manifest never retired them; the design's
  cleaner HAND is a candidate follow-up).
- 2026-08-13: Pack 2 lands. verify.sh green (756 → 759: +2 render units,
  +1 http). Browser-smoked: wheel tap → sheet → PLAY → staged TABLE
  overlay, badge READ→TARGET→PLAY, pull count live, drawer slides.
  Interim note: until Pack 3, the Draw beat has no swap UI (tap-to-swap
  retired with `lc:arm`; the action-bar hints still mention it — Pack 3
  re-words them onto the MULLIGAN button).
- 2026-08-13: Pack 1 lands. All seven items shipped; verify.sh green
  (workspace 751 → 756: +3 render units, +3 http, −1 retired picker
  test). Browser-smoked on a live 2-player room at Diplomacy: tray →
  overlay → arm flash → stack → persistent arrow → take-back all match
  the prototype. Deviations, both deliberate: flash/persistent arrows
  carry no arrowhead marker (SVG markers can't take per-deck ink without
  context-fill; revisit in Pack 3's felt polish if wanted), and the
  centre "PLAYS n" caption is Pack 3's item, not this one's.
