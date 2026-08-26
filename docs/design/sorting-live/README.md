# Live load board — working demo

A clickable board for loading the van when there is no generated plan. Open it,
tap through it, and switch between three levels of scanning to see how much
foresight each one buys.

## Running it

The demo is one self-contained file. No build, no server, no dependencies:

```bash
xdg-open docs/design/sorting-live/demo.html     # or just double-click it
```

If you would rather serve it (handy for testing from the tablet on the same
network):

```bash
npx http-server docs/design/sorting-live -p 8080     # then http://<your-ip>:8080/demo.html
```

It is authored at exactly **1440 × 840 CSS px** — the Movink Pad Pro in
landscape, after Chrome's URL bar. A narrower window scales the whole board
rather than reflowing it, so what you tap here is what you tap there.

## The three test cases

The buttons along the top swap what was known before the doors opened. The
board, the state and your progress stay the same — only the foresight changes.

| Case | What you gave it | What it can tell you |
|---|---|---|
| **Customer order only** | the route list | nothing until you tap it in: every count, position and stack height is decided at the pallet |
| **Order + crate counts** | plus a total per customer | the whole van planned up front — every empty position shows, in blue, who belongs there and how many |
| **Everything scanned** | plus which pallet each order is on | the console also names the pallet to pull from next |

The plan the middle case draws is produced by running the *live* rules forward
over the known counts — same window, same split, same fill order — so a route
sorted with a manifest and one sorted blind end up in the same van.

## What to try

- Tap **+ 1 crate in** four times, then **Done · Jåtten**. Five taps for a
  four-crate stop, four of which are the crates.
- Look at the three side spots: one green, two amber. The amber ones say which
  tap clears them.
- Tap an empty position the side door can still reach — it becomes the target,
  and the board says what the gap will cost.
- Switch to **Order + crate counts** and watch the empty positions fill with
  blue ghosts.
- Hit **Start over** to reset.

## Rebuilding

```bash
node docs/design/sorting-live/build.mjs      # src/ -> demo.html
node docs/design/sorting-live/src/model.test.js
node docs/design/sorting-live/src/board.test.js
```

`src/model.js` is the whole rule set — fill order, the ±3 window, splitting,
combining, the doors, the doorways. `src/board.js` turns it into what the
screen shows. `src/board.html` is the markup, shared verbatim with the design
canvas; `src/runtime.js` is the ~70-line template runtime that renders it, so
the demo and the design cannot drift apart.

The two test files run on plain `node` with no dependencies and cover the rules
and the rendering respectively.
