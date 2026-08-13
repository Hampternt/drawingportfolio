# Last Call: screen declutter + installable app

**Status:** done — pack gate green 2026-08-13, browser-verified
**Branch:** `feat/last-call-refinement`

User-specified items (2026-08-13 playtest feedback): the phone screen is
crowded — pact propose buttons, the drink-registration block, handicap
rows and the container field all go; the browser bar should be dodgeable.

## Items

- [x] **1. Retire the pact UI** — the PROPOSE/ACCEPT/DECLINE section stops
      rendering on the hand fragment. Engine + routes stay (dormant, no
      button ever posts to them) so the mechanic can return in a v2
      design; the public break strip remains. (4ff9ebc)
- [x] **2. Slim the lobby setup** — the "Your drink"/"Handicaps" block
      becomes one register row (deck select + button). Container input
      gone (people know their own glasses); handicap rows gone (mechanic
      dormant, engine + route kept — reverses spec §2 item 2's page-level
      property); wait-line dropped from the phone (the table felt and big
      screen still carry the roll call). (a530d41)
- [x] **3. Installable app (browser bar)** — web app manifest + embedded
      icons + apple meta tags on every phone page; Add to Home Screen
      launches standalone, no browser chrome. That's the only method
      phones allow. (87f3509)

## Later (noted, not in this pack)

- Hand screen becomes a pure card-reading surface; playing/targeting moves
  to the table screen — user will bring UI design docs first (same bucket
  as the Reveal/Resolve visual passes).

## Ledger

- 2026-08-13: pack opened from playtest feedback; items are user-specified.
- 2026-08-13: items 1–3 landed, one commit each; pack gate green (clippy
  at the documented 21; workspace at 751 tests — 3 retired with the pact
  section, CLAUDE.md updated).
- 2026-08-13: browser walkthrough on the dev server (test mode, room
  BKSB): lobby hand pane is the register row alone — no container input,
  no handicap rows; caught and fixed the row's select ballooning to the
  wheel-wide pane (`flex: 1` dropped); Diplomacy shows no pact section
  and the TALK IT OUT / LOCK IN bar; `/assets/manifest.json` +
  icons serve 200 and every phone page links them.

<details>
<summary>Deviations & notes</summary>

- Drink registration could not be removed outright — a registered drink
  is what deals you a hand and gates START ROUND 1 — so it shrank to one
  row. If it should vanish entirely, dealing needs a different rule
  (user's call).
- Handicaps and pacts are now UI-less but engine-complete: routes answer,
  state fields persist, tests pin the dormant behavior. Full excision is
  a separate pack if ever wanted.
- Spec §2 item 2 ("any member sets any handicap") page-level test replaced
  by the one-register-row shell test; the engine-side property still holds.

</details>
