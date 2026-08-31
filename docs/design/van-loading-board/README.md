# Van loading board — design document

`Van loading board.dc.html` is a design document for the live loading board, on
the Hampter design system. Open it in Claude Design, or run `node preview.mjs`
and open `.preview.html` in any browser.

## What is in it

| | |
|---|---|
| **S1** | The board mid-load, route list only |
| **S2** | The same board with the crate counts scanned, so the forecast has something to say |
| **S3** | Later, with the side door shut and the last stop packed at the back |
| **S4** | The projection — basis vectors, where they put things, and how much a nearer stack hides |
| **S5** | Every state a push button can be in, with the sentence it shows |
| **S6** | Starting a stop: the two rail buttons in every state they have |
| **S7** | What changed from the plan view, and what it cost |
| **S8** | The three places it departs from the design system, and why |

## The boards are not drawings of the screen

They are the screen. `ssr.mjs` renders `../sorting-live/src/board.html` — the
markup the prototype itself runs — against the value tree `board.js` produces,
from states built by tapping the rule set in `model.js`.

```bash
node build.mjs      # -> Van loading board.dc.html
node preview.mjs    # -> .preview.html, the same markup without the canvas runtime
```

`ssr.mjs` supports exactly what the prototype's 70-line `runtime.js` supports and
nothing more: `{{dotted.path}}` in text and attributes, `<sc-for list as>`,
`<sc-if value>`, and `onClick`, which a static page drops. It has no dependencies
and needs no browser.

**The two are checked against each other.**

```bash
node ssr.test.mjs   # needs a browser, unlike the prototype's own tests
```

It renders the same state both ways and compares node for node — tag, style
attribute and text, all 205 of them. They match exactly, so a design document
that disagreed with the prototype would be a test failure rather than something
to notice later.

Every figure, name, code and explanatory sentence on the page comes out of that
same code, with 316 passing checks behind it. That is why S5 and S6 are worth
trusting: those are not sample sentences, they are what the board says in those
fourteen situations — and writing them out is how the fixtures found a real bug,
a full van offering to carry a stack round to a door with no floor behind it.

## Screenshots

`shot-S1`, `shot-S2`, `shot-S5` and `shot-S6` are checked in for reference — one
board, the same board with the forecast on it, and the two state tables, which
are the parts worth seeing without opening the file. They are regenerated, not
edited.
