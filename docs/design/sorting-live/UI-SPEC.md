# Van loading board — UI specification

**A brief for redesigning this screen. Self-contained: assume no other context.**

Everything below is either a physical fact about the van, a rule the driver
gave, or a measurement taken from a working prototype. Where something is a
weakness rather than a requirement it is marked **WEAK** — those are the parts
worth redesigning. Where something is load-bearing it is marked **FIXED** —
changing it breaks the van, not the layout.

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

## 4. Plan-view orientation — FIXED

The map is a plan view, looking down.

**Landscape:** cab at screen **left**, back doors at screen **right**. A van
pointing left has its **right side at the top of the view**, so:

- the **upper band is the van's RIGHT (kerb) side**,
- the **lower band is its LEFT (driver) side**,
- the side door, being on the right, has its packing area **above the map,
  pushing down** into rows 1–4,
- the back spots sit **off the right-hand end, pushing inward**.

**Portrait:** cab at the **top**, rows running down. The van's right is then
screen-right, so the packing area becomes a **column beside rows 1–4**, and the
back doors are at the **bottom pushing up**.

Getting this backwards was the first thing the driver corrected. It is not
decorative — they read the screen as a picture of the van in front of them.

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

Measured from the working prototype. Heights are what currently fits; the
proportions are the point, not the exact numbers.

```
 14  page padding
 44  header          route name · four stats · odd-crate flag · view toggle
 76  console         the direct-load control strip
 18  side-door line  status and budget
146  packing spots   3 tiles across
 14  row numbers     R1 … R9 + "BACK DOORS"
146  band: RIGHT     row label · 9 cells · arrow · back spot 1
146  band: LEFT      row label · 9 cells · arrow · back spot 2
  —  conditional     capacity warning (34) and/or doorway strip (54), both hidden by default
166  load order      six stop chips, reverse delivery order
 14  page padding
```

Fixed rows total **674px**, leaving 166 for the load-order strip. With both
conditional strips showing it drops to **62px**. **WEAK — there is no slack;
see §10.**

Horizontally the band is: row label 52 · nine cells at **111px** · arrow 20 ·
back spot **244px**, with 8px gaps.

### 6.2 Portrait — 900 × 1384

Same components, regrouped by row instead of by column. Two zones split at the
door boundary so the packing spots sit beside exactly the rows they can reach:
rows 1–4 with the side column on the right, rows 5–9 with the route strip
alongside, back spots below. Rows come out ~90px each.

**WEAK: portrait was derived from landscape by script, not designed.** It is
correct but it has never been through a layout pass of its own.

### 6.3 The console — the single most-used control

Most crates never touch a packing spot: they come off the top of the pallet and
go straight into the van. That path lives here, in one fixed strip, with the
largest targets on the screen, so the hand never hunts for a tile.

| Element | Size | Content |
|---|---|---|
| eyebrow | — | `STRAIGHT OFF THE PALLET → R3 · R · SIDE DOOR` — tier 3 replaces the first clause with `PALLET C →` |
| title | 24px | the customer the loading order says is next |
| sub | 15px | `stop 5 of 6 · 3 stacked here` (+ ` · 5 expected` at tier 2+) |
| **`+ 1 crate in`** | **212 × 56** | the primary. One tap per crate placed. |
| `−` | 56 × 56 | miscount |
| `Full · next position` | 186 × 56 | seals this position, opens the next |
| **`Done · <name>`** | 176 × 56 | closes the customer out, advances the queue |
| `Undo` | 92 × 56 | one step back, always |

**Target loop: five taps for a four-crate stop** — four of them the crates
themselves.

### 6.4 A packing spot

A part-built stack standing on the floor by a door. Five of them; each holds one
customer at a time.

```
SIDE 1                                    → R3 · R
[ 66×54 count — tapping it is +1 ]  Jåtten Skole        [ − ]
                                    stop 5 of 6 · staged
[ push button, 44 tall, flex ] [ alt, 82 ] [ Done, 74 ]
```

- Tapping the **count** is +1. On an empty spot the first tap both takes the
  next customer *and* counts the first crate.
- The **sub-line** carries the reason whenever the push button is not plain
  green. This is where all the explanatory copy lives.
- The **alt button** appears only when there is a genuine second option (§7).

### 6.5 A van position

18 of them. At 111px wide this is the tightest component on the screen.

- Header: column letter (`L`/`R`) and a status pill.
- A vertical **stack column** showing crates from the bottom up, coloured by
  customer, hidden entirely on untouched positions.
- Head: `5 free` (space view) or the customer (identity view).
- Sub: `3 crates`, or `two customers` when mixed, or `send here` when it is a
  legal manual target.

Pills: `IN` · `MIXED` · `OPEN` · `NEXT` · `PICKED` · `PLANNED` · `EMPTY` · `SHUT`

### 6.6 The doorways

The side-door well and the back doorway are floor you can stand a stack on, off
the numbered grid. **Hidden until they matter** — they appear when the floor
runs short or one is in use.

- Whatever stands in a doorway is the first thing in the way at every stop, so
  the board checks whether it holds the **earliest delivery still to load** and
  says so either way.
- The side well is **shared with the freeze ware**, which goes in at the end. A
  toggle on the tile carries that fact rather than pretending the space is empty.

### 6.7 The load-order strip

Six chips, reverse delivery order — the order things go in. Each shows the
customer, where their crates ended up, and a state: `DONE ↺` (tappable to
reopen) · `LOADING NOW` · `ON SIDE 2` · `WAITING`.

At the end of the session this strip is the artefact the driver carries into the
round: it says where every stop's crates are.

**WEAK: 166px of height for three short lines. This is the emptiest region on
the screen and the most obvious place to earn something back.**

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
2. **A position of its own.** The safe default.
3. **The doorway**, once the floor is running short.
4. **On top of another customer.** Last, and only when forced.

**Mixing two customers on one stack is how the wrong goods get carried into a
building.** It is never the quiet default. It appears only when the stability
rule leaves no room or the van has genuinely run out; it goes in amber, not
green; and it says what it costs. A position that ends up holding two customers
wears `MIXED` in its pill so it is visible from across the van at delivery time.

When a combine does happen: the **later-delivered customer goes underneath**,
the earlier one on top, so the earlier one comes off without disturbing them.

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
| ready | green | `Push in → R1 · L` | — |
| out of order, blocker staged | amber | `Push in anyway` | `Olavstoppen goes in first — they are on SIDE 2.` |
| out of order, blocker aboard | amber | `Push in anyway` | `Tap Done on Olavstoppen first, or push this anyway.` |
| out of order, blocker nowhere | amber | `Push in anyway` | `Olavstoppen has not been staged — push this and it lands in front of them.` |
| too tall for the window | amber | `Push in 4 of 12` | `All 12 will not stand at R2 · L — 5 is its ceiling. Split it 4 + 4 + 4: R2 · L, then R2 · R, then R3 · L.` |
| too thin for the window | amber | `Push in anyway` | `Only 1 next to R1 · L’s 8 — 7 apart, and 5 is the floor here.` |
| side rows full, 2+ crates | red | `Round the back` | `Rows 1–4 are full, so nothing more goes in this way — it would have to travel past what is already aboard. Carry this round to the back.` |
| side rows full, 1 crate | green | `Put it at the side door` | `One crate — the side door is the easy place to reach it from, and it keeps Jåtten Skole off anybody else’s stack. The freeze ware shares this space at the end.` |
| position hand-picked | amber | `Push in → R4 · R` | `You picked this one. R1 · L stays free — whoever fills it ends up deeper.` |
| van and doorways full | red | `Van full` | `Every position and both doorways are taken. Nothing left to put it in.` |

**FIXED — the shape of this:** amber means *allowed, and here is what it costs*.
Red means *the van physically cannot*. Only two things are ever red. Every
non-green state names the tap that clears it.

### 7.5 Warnings

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

1. **The band's end tile must not flex.** Given `flex: 1 1 0` beside nine cells
   it collapsed to 126px with its text column at **zero width** — present, but
   empty. Fixed width at the end of a band.
2. **"5 free" truncates to "5 fr…" at 111px** if the numeral stays 21px. Drop it
   to 17px, and drop the position label to its column letter — the row number is
   already in the header above.
3. **Eighteen positions each drawing eight dashed capacity slots reads as graph
   paper.** Draw the column only where there is something to show.
4. **The side well is reachable *after* rows 1–4 fill, not before.** The obvious
   inference is backwards: that is exactly when it becomes useful.
5. **Same-row pairs are not order-free.** It looks safe to let two customers
   share a row in either order, but the one being skipped is by definition
   unfinished — their next crates go one row further out, and the other stack is
   then deeper than part of them.

---

## 10. What to make better

The rules in §3, §4 and §7 are settled. Everything here is open.

1. **The map is cramped and the strip below it is empty.** Nine positions across
   1440px leaves 111px each, which forces three-letter codes and a 17px numeral;
   meanwhile the load-order strip has 166px for three lines of text. The
   information is in the wrong proportions.
2. **No slack anywhere.** 674px of fixed rows in 840. Three conditional warning
   strips can all appear at once and squeeze the route strip to 62px. A layout
   that degrades gracefully under those would be a real improvement.
3. **Colour is carrying identity alone.** Six customers is fine. A twelve-stop
   route with two Rema 1000s and three Coops is not — codes help, but the stack
   columns are pure colour.
4. **"How full is the van" and "who goes where" are currently a toggle.** They
   are the two questions the driver has, and switching between them is friction.
   One view that answers both would be better.
5. **Portrait has never had a design pass** — only a mechanical derivation.
6. **The planned-vs-actual distinction is one blue tint.** At tier 2 the board
   knows what should happen and what did; that difference could be made much
   more legible.
7. **Nothing signals a skipped tap.** If the driver stops tapping for two
   customers, the board shows a confidently wrong van with no signal anywhere.
   The app has no independent view of the vehicle, so it cannot detect this — but
   a design that makes the map feel like a record being kept, rather than a fact,
   would set the right expectation.

---

## 11. Reference implementation

A working, tappable version of everything above is in this folder:

```
demo.html          open it directly — no build, no server, no dependencies
src/model.js       every rule in §3 and §7, as ~600 lines of plain JS
src/board.js       the rules turned into what the screen shows
src/board.html     the markup
src/*.test.js      273 checks, run on plain node
```

Use it to check behaviour, not to copy layout — the layout is the part that
needs the work.
