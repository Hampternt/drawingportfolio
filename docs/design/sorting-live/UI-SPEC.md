# Van loading board — UI specification

**The specification for this screen. Self-contained: assume no other context.**

Everything below is either a physical fact about the van, a rule the driver
gave, or a measurement taken from the working prototype in this folder. Where
something is a pixel budget rather than a requirement it is marked **WEAK** —
those are the parts free to change. Where something is load-bearing it is marked
**FIXED** — changing it breaks the van, not the layout.

---

## 1. What this is

A grocery wholesaler delivers to schools, nurseries, canteens and shops around
Stavanger, Norway. Goods travel in stackable plastic crates (IFCO). Each morning
a driver is given a route of six to twelve stops and a pallet of mixed crates,
and has to load a van so that at every stop the right crates are reachable
without unloading anything else.

This screen is the tool for that half hour. It is held or propped at the pallet,
beside the open van, and tapped between lifts. It is **not** a planning tool
used at a desk — every tap happens with a crate in the other hand.

There is a second, older mode where an assistant reads the pallet photos and
generates a complete plan in advance; this brief is about the **live** mode,
where little or nothing is known before loading starts.

---

## 2. Who is holding it, and where

- **Device: Movink Pad Pro**, 2880 × 1800 physical, device pixel ratio 2.
  **FIXED: the design canvas is 1440 × 840 CSS px** — 900 minus Chrome's URL
  bar. Portrait is 900 × 1384.
- Standing, in a warehouse, often cold. Frequently gloved, so capacitive contact
  is intermittent: **a dropped long-press silently degrades to a tap.**
- One hand is usually holding a crate. Reaching across the screen costs a step.
- The tablet is propped or cradled, not held in both hands.

**FIXED — consequences of the above:**

- **No long-press, no drag, no swipe.** Any of them can misfire into a tap, and
  a misfired tap here commits a crate to a position.
- **No timed affordances.** No "undo available for 20 seconds", no toast that
  expires. A walk to the pallet takes longer than any reasonable timer.
- **No modal dialogs and no confirmations.** Every action must be reversible
  instead of confirmed.
- **Minimum target 48 × 48 CSS px** (Material's minimum at this DPR). The
  primary action is far larger — see §8.
- Everything needed for the common case must be reachable without scrolling.
  **The board does not scroll.** It fits 840px or it does not ship.

---

## 3. The van — FIXED, all of it

This is physical reality, verified against the vehicle. None of it is a
preference.

| | |
|---|---|
| Load bay | **9 rows front-to-back × 2 columns** (left / right) = **18 stack positions** |
| Stack height | **8 crates** (roof) |
| Side door | On the **kerb side** (right, in right-hand-traffic Norway). The driver's side has no door. |
| Side door reach | **Rows 1–4 only.** Past those, a stack would have to travel over what is already aboard. |
| Side door closure | Once rows 1–4 are full, nothing more goes in that way. The rest is carried round to the back. |
| Back doors | Twin barn doors. Serve **rows 5–9**. |
| Packing spots | Small patches of floor beside each door where part-built stacks stand. **3 at the side, 2 at the back** — though the driver says 2 at the side. **UNRESOLVED, treat as a setting.** |
| Doorways | The side-door well and the back doorway are themselves floor. Off-grid, last resort. See §6.5. |

**Loading order is the reverse of delivery order.** The last stop delivered is
loaded first and ends up deepest, against the cab. The first stop delivered sits
nearest the doors and comes out first.

**The stability rule, stated exactly:**

> Within **each column independently**, front to back, a stack may be at most
> **3 crates taller or shorter** than the stack immediately in front of or
> behind it in that same column.

Three things about it that are easy to get wrong and all three matter:

1. **Left and right in the same row are never compared.** They may differ by any
   amount. The rule is front/back within one column, only.
2. **An empty position is exempt.** The rule applies only between two positions
   that both actually hold a stack.
3. A position whose contents were never counted is exempt too — the app does not
   know its height and must not pretend to.

**Fill order — FIXED:**

```
r1-left, r1-right, r2-left, r2-right, … r9-left, r9-right
```

Rows ascend because nothing can be put in front of what is already aboard.
Within a row, **left before right**, because the door is on the van's right: the
far column has to be crossed to, and a filled near column blocks that path.

---

## 4. The view — FIXED

The board draws the van **from its own rear-right corner, raised** — the corner
the driver is standing at with the back doors open and the kerb on their right.

It is a parallel projection, so every position is the same size and stack
heights can be compared across the whole van by eye. It is **dimetric, not true
isometric**, and it has to be: at 30° on both axes, nine rows of van is 950px
wide and 850px tall before a single crate goes in, which does not fit a 1440 ×
840 board that also has to hold a queue.

Two floor basis vectors and one vertical, in screen pixels before scaling:

| axis | direction | vector |
|---|---|---|
| `u` — one column, left wall → right wall | right and **down** (nearer) | `(+120, +50)` |
| `v` — one row, cab → back doors | left and **down** (nearer) | `(−42, +48)` |
| `w` — one crate of height | straight up | `(0, −11)` |

`P(u, v, w) = (u·120 − v·42,  u·50 + v·48 − w·11)`, then scaled by one factor `k`
so the whole picture fits its box. The projection is linear in `k`, so the
bounding box is too — the fit is one division, not a search.

**What that buys, and it is the reason for the whole view:**

- The cab-left corner is the top of the picture; the back-right corner — the one
  you are standing at — is the bottom.
- **The left column is on the left and the right column is on the right.**
- The side door, which only ever serves rows 1–4, ends up **at the far end on
  the right**, with its packing spots on the pavement outside it.
- The back doors frame the near end, with their packing spots on the ground
  beyond.
- **Stack height is drawn as height.** No gauge, no bar, no legend — the van is
  a terrain and the ±3 rule is why it reads as a staircase rather than a wall.

**Paint order is depth order.** Cells are emitted back to front (`row + column`
ascending), so a nearer stack paints over what it stands in front of, exactly as
it does in the van. There is no `z-index` anywhere.

**Occlusion is real and is bounded by the rule.** One crate of height is 11px
against a row step of 48px, so a stack the maximum legal three taller than the
one in front of it still shows a 15px sliver of its top face. A stack that hides
its neighbour entirely is a stack that broke the ±3 rule.

**Nothing with words in it is ever sheared.** Floor tiles and stack faces carry a
`matrix()`; every label is a separate upright chip positioned at a projected
point. Sheared text is unreadable at arm's length in the rain.

**The `−1` in the kerb face's matrix is load-bearing.** The face toward the right
wall is `matrix(-1, ry/rx, 0, 1, e, f)` on a `rx`-wide div; `skewY()` cannot
produce it at any origin, and substituting one mirrors every right-hand face.

**Scale.** The picture is height-bound: `k = min(1, SCENE.w/bw, SCENE.h/bh)`
comes out **0.982** in a 1014 × 734 box, giving a 10.8px crate and a 118px
column. `k` saturates at 1.0 once the box is 748 tall, so there is nothing to
gain past that.

---

## 5. Three levels of foresight

The same board, three different amounts known before the doors opened. The UI
must work at **tier 1** and merely get better at 2 and 3.

| Tier | Input | What the board can say |
|---|---|---|
| **1 · Route only** | an ordered stop list | nothing until it is tapped in |
| **2 · + crate counts** | plus a total per customer | the whole van planned up front; every empty position shows who belongs there and how many |
| **3 · + pallet contents** | plus which pallet each order is buried in | also which pallet to pull from next |

**FIXED:** tier 2's plan is produced by running the *live* rules forward over the
known counts — same stability window, same split, same fill order. A route
sorted with a manifest and one sorted blind must land in the same van.

---

## 6. Screens and components

### 6.1 Landscape anatomy — 1440 × 840

Three regions, and the third one stands inside the second.

```
┌──────────────────────────────────────────────────┬────────────────┐
│ ← route · date · stats            ╱▔▔╲  cab      │ LOADING ORDER  │
│                              ╱▔▔▔╲     ╲         │                │
│                         ╱▔▔▔╲ van ╲     ╲  ┌SIDE1┤ ▸ Olavstoppen  │
│                    ╱▔▔▔╲      ╲     ╲    ╲ ┌SIDE2┤ ▸ Jåtten       │
│               ╱▔▔▔╲     ╲      ╲     ╲   ╱ ┌SIDE3┤ ▸ Hinna  side│move
│          ╱▔▔▔╲     ╲     ╲   ┌──────────┐        │ ▸ Sverdrup     │
│     ╲▁▁▁╱ back doors ╲    ╲  │  DOCK    │        │ ▸ Frøystad     │
│  ┌BACK1┐┌BACK2┐       ╲▁▁▁╱  └──────────┘        │ ▸ Marlink      │
│                                                  │  ⚠ warnings    │
│                                                  │ [⚑ odd][❄]     │
└──────────────────────────────────────────────────┴────────────────┘
```

| region | box | why there |
|---|---|---|
| header | `24, 14` — over the top-left dead corner | the diagonal never reaches it |
| picture | `16, 70, 1014 × 734`, scaled to fit | the diagonal |
| **dock** | `566, 520, 464 × 284` | see below |
| queue rail | `right 16, top 14, 376 × 812` | the hand that is not carrying a crate |

**The dock stands on the pavement, inside the picture.** Invert `P` on its
top-left corner and it lands at roughly `(u, v) = (3, 5.5)` — kerb-side ground
aft of the side door and outboard of the back doors, between the two clusters of
packing spots and touching neither. That is where a driver stands with the back
doors open.

**WEAK — the pixel budget.** **FIXED — that nothing is ever drawn into the dock.**
The clearance is single digits: an eight-high stack at R6·R reaches x = 557
against the dock's left edge at 566, and a full pile on SIDE 3 bottoms out at
y = 516 against its top edge at 520. `board.test.js` asserts it with all
eighteen positions stacked to the roof and all five spots piled high, using a
separating-axis test on the real parallelograms — a bounding box says the floor
slab covers the dock when the slab itself is nowhere near it.

### 6.2 The queue rail — the way in

The route in **loading order** (reverse of delivery), one row each. Every row
carries two buttons, and choosing between them is the only decision the board
cannot make for the driver:

| row state | buttons |
|---|---|
| waiting | `side` `rear` |
| being packed at the side | `side` (lit) `move` |
| aboard, closed out | `reopen` |

`move` carries a part-built stack round to the other door, crates and all. It is
what a door shutting mid-order actually calls for.

Rows flex to fill the rail — six stops give 96px rows, a fifteen-stop route gives
62px — so a short route is not a wall of small targets.

### 6.3 The dock — the single most-used control

Two rows, tethered by a line to whichever packing spot it is driving, and
bordered in that customer's colour. The user asked for the push button to be
*on the packing area the customer is being packed in*; a control that jumps
between five pads is a mis-tap generator, so it is one fixed control that says,
visibly, which pad it belongs to.

**Row A — the loop, run a hundred times a load. These never move.**

| control | width × height | does |
|---|---|---|
| `−` | 56 × 76 | takes a crate back off the spot |
| `+ 1 (n)` | 128 × 76 | counts one onto the spot |
| **`Push in n`** | **238 × 76** | commits the spot's pile to one van position |

**Row B — everything that ends something.**

| control | width × height | does |
|---|---|---|
| `+n on top` | 208 × 60 | puts the pile, or one crate of it, on an existing stack |
| `Done` | 112 × 60 | closes the stop out, hands the dock back |
| `Undo` | 102 × 60 | steps the whole board back one action |

`Done` sits **246px from `Push in`**, on a different row, at half the height and
in a different colour. There are no confirmations anywhere on this board, so
separation is the only guard against the one action whose consequence is not
visible on the van.

**A push with nothing counted is allowed and is never green.** It records an
unknown, not a zero, and the sub-line says `R4 · R · not counted`. Tapping `+ 1`
until the number is right turns the same button green. That is the whole bargain
of the fast flow: you may load blind, and the board will not pretend it knows
what you loaded — the headline stat switches from `CRATES IN` to `POSITIONS IN ·
n blind` the moment anything goes in uncounted.

The dock is **226px tall with nothing to explain and 284px with**, growing
downward only — the buttons are top-anchored, so nothing under a reaching hand
ever moves.

### 6.4 A packing spot

An unwalled pad of ground outside the van, drawn where it physically is: three
along the right flank beside the rows the side door reaches, two behind the back
doors. **The pile standing on one is drawn at true crate height**, inset inside
the pad so the pad stays visible as ground — so the stack you have built and the
stack it is about to become are the same picture.

Its name sits on the pavement beside it, never in front of it: the spots are laid
out along one axis and a label placed "in front" of one lands on the next.

Tapping a pad hands it the console.

### 6.5 A van position

A floor tile, and on it a stack drawn as three faces — the one facing the back
doors, the one facing the right wall, and the top. Individual crates are a
repeating gradient inside each customer's band, so an eight-high stack of two
customers is four divs, not eight.

One upright chip per position, riding the **back** edge of the top face where the
row in front cannot cover it: `CODE n`, or `CODE+CODE n` when a stack holds two
customers. `?` when it went in uncounted.

The next position in is filled with the accent and labelled `NEXT IN`; a
hand-picked one says `PICKED` in amber. With counts known, an empty position
draws its planned stack as a **translucent blue volume** — a face down to the
floor, not a lid hanging in mid-air — and the next position merges the two into
one chip, `NEXT · HIN 2`.

### 6.6 The proposed top-up, drawn where it would land

Not a word on a button: the crates themselves, in the customer's colour at half
opacity with a dashed amber edge, standing on the host stack. `+2`. Tapping
another legal stack moves them there.

### 6.7 The slide

A stack that has just been pushed in arrives from the spot it was built on:
260ms, `cubic-bezier(.22,.61,.36,1)`, over 95–342px depending on the pad.

The mechanism matters, because the obvious one is unimplementable here. The
runtime rebuilds the entire tree on every paint (`host.textContent = ''`), so a
JS-held clone is destroyed about 16ms into any transition. The one that works
needs no JS at all:

```css
@keyframes sc-push { from { translate: var(--dx) var(--dy); } to { translate: 0 0; } }
```

`translate` is an independent transform property that composes **outside**
`transform`, so the stack's shear matrix is untouched, and a freshly created
element always starts its animation — the full re-render is what *drives* the
motion rather than what breaks it. The keyframe lives in the helmet, the only
place in this document where a style block belongs.

**It is decoration on a board that is already correct.** The model commits at
tap time, and the target position carries its accent outline *before* the tap,
so a driver whose eyes are on the crate never depends on seeing it. The flash is
consumed on the first paint without a `setState`, or every later repaint would
replay it.

### 6.8 The van shell

Only the two walls that stand **behind** the load are drawn full height — the
left wall and the cab bulkhead — each with a lit top rail. The right wall is
between the camera and everything it holds, so it is cut down to a sill, and
**the side door is the stretch of that sill it opens through**: amber and
knee-low across rows 1–4 while it is open, red when rows 1–4 fill and it shuts.
No roof. Row numbers run down the outside of the left wall.

---

## 7. The decision rules — FIXED

This is the part that must survive a redesign intact. It is what the driver
actually asked for.

### 7.1 Where a pushed-in stack lands

The innermost position the door can still reach. Side spots serve rows 1–4;
back spots serve rows 5–9. A stack never crosses between zones.

### 7.2 Priority order for a stack that needs a home

1. **A single crate → the side door well.** Easy to reach, and it keeps that
   customer off anybody else's stack.
2. **A position of its own.** The safe default, and always the green button.
3. **The doorway**, once the floor is running short.
4. **On top of another customer.** Last, and only when forced.

**Mixing two customers on one stack is how the wrong goods get carried into a
building.** The top-up button is always present, because a driver with two
crates and a good reason should not have to fight the interface — but it is
**quiet, never green**, and it turns amber only when the stability rule leaves no
room or the van has genuinely run out. When it does, the console says both
things: why the ordinary push is amber, *and* what the remedy costs. Saying only
the first is how a driver ends up mixing a stack without being told what mixing
is.

**Topping up the same customer's own stack is not mixing** and is always
available — a leftover crate going back where the rest of that order already is
is the small-order case the driver asked for, not a compromise.

When a combine does happen: the **later-delivered customer goes underneath**,
the earlier one on top, so the earlier one comes off without disturbing them.
The board will not offer it the other way round, and says so:
`Olavstoppen is delivered before everything aboard on this side, so they would
have to go underneath. Give them their own position.`

### 7.3 Splitting a big order

Divide it **evenly across the fewest positions that will hold it**, front-most
positions taking the remainder. The spill goes to the next position in fill
order — usually the same row's other column, which is free floor because ±3
never compares left to right.

**Do not "fill to the ceiling and spill the rest".** Behind a closed 8 that
produces 8 then 2 — a gap of six — manufactured by the mechanism that claims to
enforce three.

### 7.4 Push-button states, with exact copy

| State | Colour | Label | Sub-line |
|---|---|---|---|
| ready, counted | green | `Push in 2` · `R4 · L` | — |
| ready, uncounted | quiet | `Push in` · `R4 · R · not counted` | — |
| **would break depth order** | amber | `Push in anyway` | `Olavstoppen is at R5 · L and comes out at stop 6. Putting Jåtten Skole deeper than them at R1 · L means moving Olavstoppen to reach the other.` |
| out of order, blocker still on a spot | amber | `Push in anyway` | `Olavstoppen goes in first — they are on SIDE 2.` |
| out of order, blocker not staged at all | amber | `Push in anyway` | `Olavstoppen has not been staged — push this and it lands in front of them.` |
| too tall for the window | amber | `Push in 5 of 10` | `All 10 will not stand at R2 · L — 8 is its ceiling. Split it 5 + 5: R2 · L, then R2 · R.` |
| too thin for the window | amber | `Push in anyway` | `Only 3 next to R3 · L’s 7 — 4 apart, and 4 is the floor here.  Hinna’s stack at R2 · R would take them — but two customers on one stack is how the wrong crate gets carried into a building.` |
| the window has closed entirely | amber | `Push in anyway` | …plus `No height works at R2 · L — its neighbours cannot both be satisfied. Something has to come back out.` |
| side rows full, 2+ crates, a back spot free | amber | `Carry round the back` · `to BACK 1` | `Rows 1–4 are full, so nothing more goes in this way — it would have to travel past what is already aboard. Carry this round to the back.` |
| side rows full, 2+ crates, no back spot | red | `Round the back` | as above |
| side rows full, 1 crate | green | `Put it at the side door` | `One crate — the side door is the easy place to reach it from, and it keeps Jåtten Skole off anybody else’s stack. The freeze ware shares this space at the end.` |
| position hand-picked | amber | `Push in 2` · `R3 · L` | `You picked this one. R1 · L stays free — whoever fills it ends up deeper.` |
| van and doorways full | red | `Van full` | `Every position and both doorways are taken. Nothing left to put it in.` |

**FIXED — the shape of this:** amber means *allowed, and here is what it costs*.
Red means *the van physically cannot*. **Every red state that names an action
must have a button that performs it** — the board used to say "round the back"
and offer no way to do it, which stranded whoever was mid-order when rows 1–4
filled. Red now means red: there is genuinely nowhere left.

### 7.5 Warnings

Loudest first. A depth fault is not a forecast — it is a statement about crates
that are already in the van in an order that will cost an unload:

- **Depth fault**: `Jåtten Skole at R1 · L is deeper than Olavstoppen at R5 · L, and comes out first — Olavstoppen has to come off to reach them.`
- **Side-door budget**, on the door line: `3 positions left · 3 staged here` and,
  when more are staged than can still fit, it **names them** — in loading order,
  so the ones that fit are not the ones flagged:
  `1 position left · 3 staged here — SIDE 2 (Hinna) and SIDE 3 (Sverdrup) will still be standing here when it shuts`
- **Capacity**: `4 positions left and 5 stops with nothing aboard — some of them will have to share a stack.`
- **Odd crate**: a header button for a crate that is unlabelled or belongs to a
  customer not on the route. The app cannot know; it carries the count to
  whoever asks at the depot.

### 7.6 Reversibility

- **Undo** steps the whole board back one action, unlimited, no timer.
- **Done is an assertion, not a fact** — more of that customer's crates can
  surface two pallets later. Tapping a finished stop in the load-order strip
  reopens it.
- A doorway stack can be taken back out.

---

### 7.7 Depth order — the rule the load runs backwards to produce

**Nothing may sit deeper in the van than a stop delivered before it.** This is
the whole reason loading runs in reverse delivery order, and until it was checked
directly, nothing checked it.

The sequence guard in §7.4 is a warning about the order of the *taps*, and one
press of `Done` dissolves it — so it never caught the case that matters, which is
the two doors worked in parallel:

```
Olavstoppen (stop 6, loads first)  → rear → R5 · L, Done
Jåtten Skole (stop 5)              → side → R1 · L      ← the button was solid green
```

At stop 5 you unload Olavstoppen to reach Jåtten. Side rows are strictly deeper
than back rows, the two zones fill independently, and the global "who loads next"
comparison had nothing to say about it.

Two checks, both count-free:

- `depthFaultAt(st, position, customer)` — would this push create one? Runs
  before the window checks, because it is worse than a thin stack.
- `depthFaults(st)` — scan the board. Surfaced as a persistent warning, because
  a fault created some other way (a hand-picked position, a reopened stop) must
  not go quiet.

**The sequence guard is per-door now.** A stop standing on the other door's spot
is not competing for this door's positions, and comparing against it made the
second door amber on the ordinary path — the same amber that carries "Round the
back", the split, the thin stack and the hand-picked gap. A warning that fires
when nothing is wrong stops being read.

### 7.8 Three floors under the rules

Not decisions — guards, so that state the board displays as fact cannot be
written in the first place.

- **The roof.** `doPush` and `doStack` refuse anything that would record a stack
  taller than `CAP`. Nothing in the dock offers it, but what a push records is
  read back as truth by `windowAt` and `planAhead`, so it is refused at the point
  of writing rather than trusted not to happen.
- **A window can close.** Two neighbours three apart in opposite directions — an
  8 in front and a 1 behind — leave `{lo: 5, hi: 4}`: no legal height at all.
  `lo` and `hi` keep their values so every caller's arithmetic is unchanged, and
  the crossing gets its own field, `boxedIn`.
- **Blind is not empty.** A stack pushed without a count records an unknown. A
  customer already fully loaded that way used to score zero crates aboard, so the
  forecast planned their whole order again and drew a confident blue ghost onto
  every empty position for crates that were already in the van. With no count
  there is nothing to subtract, so there is nothing honest to plan: the forecast
  says it does not know, and the headline switches from `CRATES IN` to
  `POSITIONS IN · n blind`.

---

## 8. Tokens

Dark only; this is used in a warehouse and never in daylight.

```
background        #0B0910      page
surface           #0E0C14      empty tile
surface-raised    #17141F      tile with content
line              #262232      border
line-quiet        #1A1723      border, empty tile

text-bright       #F2EEF8      headings, numerals
text              #CDC6DD      body
text-muted        #8D87A0      secondary
text-dim          #5F5876      labels, metadata
text-ghost        #4A445C      disabled

accent            #B48EF7      primary action, "next"
accent-text       #CBB0FF      accent on dark
go                #4FD6A8      allowed, done
warn              #FFB570      allowed with a cost, the side door
stop              #F7768E      physically impossible, mixed-stack danger
plan              #7AA2F7      planned but not yet real
```

Type: **Archivo** 700/800 for headings and numerals (tight, -0.02em);
**Space Grotesk** 400–600 for body; **IBM Plex Mono** 400/500 for labels,
position names and anything read as a code (`+0.06–0.10em` letter-spacing).

Radii 10–14px. Gaps 6/8/10. Customer colour is assigned per route and used as
the identity carrier in the stack columns and chips.

**Customer codes:** at 111px a cell cannot hold a name, so it falls back to a
three-letter code. **FIXED: codes must be derived from the whole route list and
disambiguated against it, never hashed per name.** "Rema 1000 Hillevåg" and
"Rema 1000 Madla" must not both render `REM`. Build the code from the word that
distinguishes them — `RHI` / `RMA` — skipping bare numbers.

---

## 9. Traps — things that were built wrong once

Worth knowing so a redesign does not rediscover them.

1. **The side well is reachable *after* rows 1–4 fill, not before.** The obvious
   inference is backwards: that is exactly when it becomes useful.
2. **Same-row pairs are not order-free.** It looks safe to let two customers
   share a row in either order, but the one being skipped is by definition
   unfinished — their next crates go one row further out, and the other stack is
   then deeper than part of them.
3. **A red state that names an action needs a button that does it.** "Round the
   back" with no way to carry it round stranded whoever was mid-order when the
   side door shut. Found by clicking through the board in a browser, not by any
   test that existed at the time.
4. **A sheared box reports nothing to a layout engine.** A `transform: matrix()`
   leaves `getBoundingClientRect` describing the untransformed box, and an
   overflow check on the parent sees nothing wrong. The only way to catch a
   stack drawn out through the frame is to multiply the four corners through the
   matrix yourself — which `board.test.js` does.
5. **`rgb(r,g,b)` with an alpha appended is not a colour.** `shade()` returned
   `rgb(...)` and the top-up ghost asked for `shade(...) + '80'`; CSS dropped the
   whole declaration and the proposal rendered as an outline with no fill. Alpha
   belongs in `rgba()`, not in string concatenation.
6. **`var` inside a per-cell closure hoists over the parameter it shadows.**
   Naming a local `plan` inside the cell loop silently made the outer `plan` —
   the whole tier-2 forecast — `undefined` for every position, and every planned
   stack stopped drawing. The test that caught it asserts the ghosts exist.
7. **Spots laid out along one axis cannot be labelled "in front".** In this
   projection "in front" is the direction the next spot is in, so every label
   landed on its neighbour. Push it out along the axis the row does not run down.
8. **Eighteen positions each drawing eight dashed capacity slots reads as graph
   paper.** It is why stack height is now drawn as height instead.
9. **A bounding box is the wrong shape to test a sheared thing against.** The
   floor slab's axis-aligned box covers most of the picture while the slab
   itself is nowhere near the dock. Use a separating-axis test on the four
   painted corners; a bbox is right for "is it inside the frame" and wrong for
   "does it hit that rectangle".
10. **`(l.n || 0)` scores an unknown as nothing.** It made `cratesIn` return 0
   with five stacks aboard, and made `planAhead` re-plan a customer who was
   already fully loaded — drawing a confident forecast over crates that were
   already in the van. Under a gesture where loading blind is normal, every sum
   over crate counts has to decide what it does about `null` explicitly.
11. **A guard that one tap dissolves is not a guard.** The loading-order warning
   goes quiet the moment `Done` is pressed, so it never caught the two doors
   worked in parallel. The invariant had to be checked on the van rather than on
   the sequence of taps.
12. **A JS-held clone cannot animate here.** The runtime rebuilds the whole tree
   on every paint, so anything holding a node across a state change is destroyed
   about 16ms in. `translate` in a keyframe composes outside `transform` and the
   re-render itself starts the animation.

---

## 10. What is still open

- **Two side spots or three.** The methodology doc says five standby spots —
  three at the side, two at the back — and that is the default. The driver once
  said two at the side. It is a dial on the start screen either way, and every
  layout in here is computed from the count rather than drawn for three.
- **A settings page for the loading rules.** The driver has asked for one:
  which priority wins, whether the side well is reserved for freeze ware, how
  many spots there are, how many rows. Everything it would control is already a
  parameter — `configure()` takes rows, capacity, side-door reach and spot
  counts — so this is a screen, not a rewrite.
- **Portrait.** The board is authored landscape and scales rather than reflows.
  A portrait artboard existed for the previous plan view and has not been
  redrawn for this one. Turning the van through ninety degrees in this
  projection is a different picture, not the same one rotated.
- **Motion.** A pushed-in stack currently appears at its position. Sliding it
  from the pad along the path it physically takes would confirm the tap without
  a word, and is the one animation this board would earn.

## 11. Reference implementation

A working, tappable version of everything above is in this folder:

```
demo.html          open it directly — no build, no server, no dependencies
src/model.js       every rule in §3 and §7, as ~830 lines of plain JS
src/board.js       the rules turned into the picture and the controls
src/board.html     the markup — {{holes}}, <sc-for>, <sc-if>, onClick, nothing else
src/runtime.js     the ~70-line template runtime that renders it
src/*.test.js      271 checks, run on plain node, no dependencies
build.mjs          src/ -> demo.html
```

Every figure and every sentence quoted in this document is produced by that
code. If the two disagree, the code is right and this document is stale.
