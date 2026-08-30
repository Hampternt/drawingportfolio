> **Superseded.** The board drawn in `Van loading board.dc.html` is the flex-band
> plan view. The live prototype has since been reworked into an isometric picture
> of the van seen from its rear-right corner, with the packing flow driven from
> the route rail and a dock standing on the pavement — see
> `docs/design/sorting-live/`, which is now the reference for both the layout and
> the rules. This document is kept as the record of the design pass that preceded
> it; do not feed it to a design tool expecting the current screen.

# Van loading board — design document

`Van loading board.dc.html` is a design document for the live loading board, on
the Hampter design system. Open it in Claude Design, or run `node preview.mjs`
and open `.preview.html` in any browser.

## What is in it

| | |
|---|---|
| **S1** | The board mid-load, route list only |
| **S2** | The same route further along, with crate counts and pallets scanned |
| **S3** | Portrait, 900 × 1384 |
| **S4** | Every state a push button can be in, with the sentence it shows |
| **S5** | What this changes from the previous draft, and the one thing it does not |
| **S6** | The two places it departs from the design system, and why |

## It is generated, not hand-drawn

```bash
node build.mjs      # -> Van loading board.dc.html
node preview.mjs    # -> .preview.html, the same markup without the canvas runtime
```

`build.mjs` imports `../sorting-live/src/model.js` — the same rule set the
working prototype runs, with 273 passing checks behind it — and renders real
board states. Every crate count, position name, customer code and explanatory
sentence on the page is emitted by that code. Nothing is transcribed, so the
design cannot quietly drift from the logic it is a design for.

That is also why S4 is worth trusting: those are not sample sentences, they are
what the board actually says in those seven situations.

## The change it argues for

The previous board asked you to switch between "how full is the van" and "who
goes where". This one answers both at once: each position carries a gauge whose
height is the fill and whose colour and three-letter code are the identity.
Where counts are known, the crates still expected are dashed rungs stacked above
the solid ones, so planned-against-actual is structural rather than a tint.

Warnings fold into the things they are about — the capacity warning is a state
of the POSITIONS LEFT figure, the side-door budget lives on the door line — so
neither is a strip that appears and shoves the layout around. The two doorways
are always drawn, because both are real floor: the rear one as a column at the
van's back across both bands, the side well at the end of the packing row.

## Why the controls are bigger than the kit

`--control-height-lg` is 42px. The primary targets here are 48–60px. This screen
is used standing at a pallet, often gloved, where a dropped tap commits a crate
to a position — so touch targets are the one place the kit is overridden.
Colour, type, radius and spacing all come straight from the tokens.

## Screenshots

`shot-S1.png` … `shot-S4.png` are rendered from `.preview.html` at the sizes the
sections are authored at, for anyone reading this without the canvas.

## Related

- `../sorting-live/` — the working prototype and the rule set
- `../sorting-live/UI-SPEC.md` — the brief this was designed against
