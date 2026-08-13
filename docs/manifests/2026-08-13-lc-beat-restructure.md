# Last Call: beat restructure — lobby handicaps, Lock folded into Diplomacy, discard/redraw

**Status:** done — pack gate green, browser-verified 2026-08-13
**Branch:** `feat/last-call-refinement`
**Follows:** [2026-08-13-lc-ready-advance](2026-08-13-lc-ready-advance.md)

User direction (2026-08-13): handicaps are a lobby concern; the Lock beat is
pointless as a separate phase once cards can be staged during table talk —
fold it into Diplomacy; add the missing discard/redraw mechanic (free-form
in round 1, once per round after). Reveal and Resolve get no mechanical
changes now but are flagged for a UI/design pass later.

## Items

- [x] **1. Handicaps → lobby only** — engine `set_handicap` gated to round-1
      Draw (the registration lobby), not every Draw; the YOUR DRINK /
      HANDICAPS panel renders only in the lobby and disappears once the
      game begins. (085de73)
- [x] **2. Fold Lock into Diplomacy** — `arm`/`disarm`/`set_target`/
      `lock_in` legal during Diplomacy; `Beat::next()` goes Diplomacy →
      Reveal (that edge now runs `reveal()` and clears pact offers);
      Diplomacy exits when every alive seat has locked (LOCK IN replaces
      READY as Diplomacy's commit; arm/disarm/target stay freely undoable
      until your own lock, which remains final); `Beat::Lock` variant kept
      in the enum for stored-blob compat but unreachable; banner renumbers
      to BEAT n OF 5. (bfebb71)
- [x] **3. Discard/redraw** — engine `mulligan()` + `POST
      /lastcall/mulligan` + Draw-beat card taps rewired from arm to
      mulligan; round 1 (lobby, once hands are dealt) unlimited; rounds ≥ 2
      once per round during Draw. (3d2d941)
- [x] **4. Tests + pack gate** — engine/route/render tests across all
      three, `./scripts/verify.sh` green, clippy at the documented 21,
      workspace at 751 tests (CLAUDE.md updated), browser walkthrough
      (see ledger).

## Later (noted, not in this pack)

- **Reveal** has no real visuals — players can't parse what was played at
  the flip or what the response window is answering. UI/design pass to
  come (user will consult design separately; expect design docs first).
- **Resolve** same — the settlement (damage, drinks owed, tab checks)
  needs a legible presentation. Same design-docs-first route.

## Ledger

- 2026-08-13: pack proposed; discard-mechanics fork put to the user.
- 2026-08-13: fork resolved — per-card mulligan (discard N, draw N free,
  each replacement from the discarded card's own deck; unlimited in round
  1's lobby, once per round from round 2). Item list then approved as-is
  ("fine for now").
- 2026-08-13: items 1–3 landed, one commit each; pack gate green.
- 2026-08-13: browser walkthrough on :3001 (hampter in browser, bob via
  curl): lobby shows setup + free-swap hint, swap logged "SWAPS 1";
  begin → Diplomacy with setup gone, TALK IT OUT + LOCK IN, no READY;
  arm + target + lock, bob locks → table flips straight to Reveal
  ("BEAT 4 OF 5", DRINK 2 + READY); both ready → round-2 Draw with
  "TAP A CARD TO SWAP IT — ONCE A ROUND"; first swap hides the hint and
  logs, a second swap that round is refused (no log entry).

<details>
<summary>Deviations & notes</summary>

- **Mulligan UI is one card per tap**, so from round 2 the practical limit
  is one *card* per round, not one batch of N — narrower than the approved
  fork text ("pick any cards"). The engine (`mulligan()`) and route accept
  batches (`cards=id1,id2`), so a batch-selection UI can lift this without
  engine changes if the single-card round limit feels too tight in play.
- Round-2+ taps are instant (no confirm) — a mis-tap burns the round's
  swap. Flagged for the later Reveal/Resolve UI design pass.
- The synthetic-click limitation from `browser_verification_limits`
  reappeared: CDP clicks don't satisfy the wheel's tap detector (travel/
  timing heuristics), so tap paths were exercised by dispatching the
  wheel's own `lc:arm` event — the identical code path a real tap runs.
- Drink re-registration UI (`YOUR DRINK` row) also vanished outside the
  lobby with the setup section; `set_vessel` stays Draw-legal engine-side.
  Surface a mid-game drink-swap UI later if it turns out to be missed.

</details>
