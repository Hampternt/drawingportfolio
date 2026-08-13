# Last Call: beat restructure — lobby handicaps, Lock folded into Diplomacy, discard/redraw

**Status:** in progress — item list approved 2026-08-13 ("fine for now")
**Branch:** `feat/last-call-refinement`
**Follows:** [2026-08-13-lc-ready-advance](2026-08-13-lc-ready-advance.md)

User direction (2026-08-13): handicaps are a lobby concern; the Lock beat is
pointless as a separate phase once cards can be staged during table talk —
fold it into Diplomacy; add the missing discard/redraw mechanic (free-form
in round 1, once per round after). Reveal and Resolve get no mechanical
changes now but are flagged for a UI/design pass later.

## Items

- [ ] **1. Handicaps → lobby only** — engine `set_handicap` gated to round-1
      Draw (the registration lobby), not every Draw; the YOUR DRINK /
      HANDICAPS panel renders only in the lobby and disappears once the
      game begins.
- [ ] **2. Fold Lock into Diplomacy** — `arm`/`disarm`/`set_target`/
      `lock_in` legal during Diplomacy; `Beat::next()` goes Diplomacy →
      Reveal (that edge now runs `reveal()` and clears pact offers);
      Diplomacy exits when every alive seat has locked (LOCK IN replaces
      READY as Diplomacy's commit; arm/disarm/target stay freely undoable
      until your own lock, which remains final); `Beat::Lock` variant kept
      in the enum for stored-blob compat but unreachable; banner renumbers
      to BEAT n OF 5; action bar and wheel/armed-column staging move to the
      Diplomacy screen. Heaviest item — the Lock-rigged test fixtures all
      shift.
- [ ] **3. Discard/redraw** — new engine action + route + hand-pane UI:
      round 1 (lobby, once hands are dealt) unlimited; rounds ≥ 2 once per
      round during Draw. Exact mechanics per the resolved design fork
      (see ledger).
- [ ] **4. Tests + pack gate** — engine/route/render tests across all
      three, `./scripts/verify.sh`, clippy budget unchanged, browser
      walkthrough.

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
  1's lobby, once per round from round 2). Item list NOT yet approved —
  user wants adjustments first.
