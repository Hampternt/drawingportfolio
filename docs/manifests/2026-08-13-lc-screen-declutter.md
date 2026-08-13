# Last Call: screen declutter + installable app

**Status:** in progress
**Branch:** `feat/last-call-refinement`

User-specified items (2026-08-13 playtest feedback): the phone screen is
crowded — pact propose buttons, the drink-registration block, handicap
rows and the container field all go; the browser bar should be dodgeable.

## Items

- [ ] **1. Retire the pact UI** — the PROPOSE/ACCEPT/DECLINE section stops
      rendering on the hand fragment. Engine + routes stay (dormant, no
      button ever posts to them) so the mechanic can return in a v2 design.
- [ ] **2. Slim the lobby setup** — the "Your drink"/"Handicaps" block
      becomes one register row (deck select + button). Container input
      gone (people know their own glasses); handicap rows gone (mechanic
      dormant, engine + route kept); wait-line dropped from the phone (the
      table felt and big screen still carry the roll call).
- [ ] **3. Installable app (browser bar)** — web app manifest + icons +
      apple meta tags so Add to Home Screen launches standalone, no
      browser chrome. That's the only method phones allow.

## Later (noted, not in this pack)

- Hand screen becomes a pure card-reading surface; playing/targeting moves
  to the table screen — user will bring UI design docs first (same bucket
  as the Reveal/Resolve visual passes).

## Ledger

- 2026-08-13: pack opened from playtest feedback; items are user-specified.
